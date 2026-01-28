<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { page } from '$app/stores';
    import { api, createWebSocket, createStatsWebSocket } from '$lib/api';
    import { user } from '$lib/stores';
    import { goto } from '$app/navigation';
    import toast from 'svelte-french-toast';
    import type { Container, ContainerPort, ContainerAllocation, Allocation } from '$lib/types';

    interface FileEntry {
        name: string;
        isDir: boolean;
        size: number;
        modified?: string;
    }

    interface ContainerStats {
        cpuPercent: number;
        memoryUsage: number;
        memoryLimit: number;
        memoryPercent: number;
        networkRx: number;
        networkTx: number;
        blockRead: number;
        blockWrite: number;
    }

    let container: Container | null = null;
    let ports: ContainerPort[] = [];
    let loading = true;
    let actionLoading = '';
    let logs: string[] = [];
    let ws: WebSocket | null = null;
    let statsWs: WebSocket | null = null;
    let logsContainer: HTMLDivElement;
    let command = '';
    let showKillModal = false;
    let sftpPassword = '';
    let sftpPasswordConfirm = '';
    let settingSftpPassword = false;
    let copiedField: string | null = null;

    // Stats
    let stats: ContainerStats | null = null;
    let statsLoading = false;
    let statsInterval: ReturnType<typeof setInterval> | null = null;

    let activeTab: 'console' | 'files' | 'sftp' | 'settings' = 'console';
    let files: FileEntry[] = [];
    let currentPath = '/';
    let loadingFiles = false;
    let selectedFile: FileEntry | null = null;
    let fileContent = '';
    let editingFile = false;
    let savingFile = false;

    // Settings editing
    let editMemory: number = 0;
    let editServerMemory: number = 0;
    let editCpu: number = 0;
    let editDisk: number = 0;
    let editSwap: number = 0;
    let savingSettings = false;

    // Allocations
    let containerAllocations: ContainerAllocation[] = [];
    let availableAllocations: Allocation[] = [];
    let loadingAllocations = false;
    let showAddAllocationModal = false;
    let addingAllocation = false;
    let settingPrimary = '';

    $: containerId = $page.params.id as string;
    $: isRunning = container?.status?.toLowerCase() === 'running';
    $: primaryPort = ports.find(p => p.hostPort) || ports[0];
    $: hasSftpPassword = !!container?.sftpPass;

    // Initialize edit values when container loads
    $: if (container) {
        if (editMemory === 0) editMemory = container.memoryLimit || 1024;
        if (editServerMemory === 0) editServerMemory = (container as any).serverMemory || container.memoryLimit || 1024;
        if (editCpu === 0) editCpu = container.cpuLimit || 1;
        if (editDisk === 0) editDisk = container.diskLimit || 5120;
        if (editSwap === 0) editSwap = container.swapLimit || 0;
    }

    let statusInterval: ReturnType<typeof setInterval> | null = null;
    let wsReconnectTimeout: ReturnType<typeof setTimeout> | null = null;
    let statsReconnectTimeout: ReturnType<typeof setTimeout> | null = null;

    onMount(async () => {
        if (!$user) {
            goto('/login');
            return;
        }
        await loadContainer();
        connectWebSocket();
        connectStatsWebSocket();
        // Poll container status every 10 seconds to detect state changes
        statusInterval = setInterval(async () => {
            await loadContainer();
        }, 10000);
    });

    onDestroy(() => {
        if (ws) ws.close();
        if (statsWs) statsWs.close();
        if (statsInterval) clearInterval(statsInterval);
        if (statusInterval) clearInterval(statusInterval);
        if (wsReconnectTimeout) clearTimeout(wsReconnectTimeout);
        if (statsReconnectTimeout) clearTimeout(statsReconnectTimeout);
    });

    async function loadContainer() {
        const wasLoading = loading;
        if (!wasLoading) loading = false; // Don't show loading on status refreshes
        try {
            [container, ports] = await Promise.all([
                api.getContainer(containerId),
                api.getContainerPorts(containerId).catch(() => [])
            ]);
        } catch (e) {
            console.error(e);
            if (wasLoading) toast.error('Failed to load server');
        } finally {
            loading = false;
        }
    }

    async function loadStats() {
        if (!isRunning) {
            stats = null;
            return;
        }
        try {
            stats = await api.getContainerStats(containerId);
        } catch (e) {
            // Stats may fail if container just started
            console.debug('Stats not available yet');
        }
    }

    function connectStatsWebSocket() {
        if (statsWs) {
            statsWs.close();
        }

        statsWs = createStatsWebSocket(containerId);
        if (!statsWs) return;

        statsWs.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                stats = data;
            } catch (e) {
                console.debug('Failed to parse stats:', e);
            }
        };

        statsWs.onerror = () => {
            console.debug('Stats WebSocket error');
        };

        statsWs.onclose = () => {
            // Only reconnect if container is running
            if (container?.status?.toLowerCase() === 'running') {
                statsReconnectTimeout = setTimeout(() => {
                    connectStatsWebSocket();
                }, 5000);
            } else {
                stats = null;
            }
        };
    }

    function connectWebSocket() {
        ws = createWebSocket(containerId);
        if (!ws) return;

        ws.onopen = () => {
            logs = [...logs, '\x1b[32m● Connected to server console\x1b[0m'];
        };

        ws.onmessage = (event) => {
            const data = event.data;
            // Handle image pull progress
            if (data.includes('Pulling') || data.includes('Downloading') || data.includes('Extracting')) {
                logs = [...logs, `\x1b[34m[PULL] ${data}\x1b[0m`];
            } else {
                logs = [...logs, data];
            }
            setTimeout(() => {
                if (logsContainer) {
                    logsContainer.scrollTop = logsContainer.scrollHeight;
                }
            }, 10);
        };

        ws.onerror = () => {
            logs = [...logs, '\x1b[31m● Connection error\x1b[0m'];
        };

        ws.onclose = () => {
            // Refresh container status on disconnect
            loadContainer().then(() => {
                // Only reconnect if container is still running and we don't already have an active connection
                if (container?.status?.toLowerCase() === 'running' && (!ws || ws.readyState === WebSocket.CLOSED)) {
                    logs = [...logs, '\x1b[33m● Console connection lost, reconnecting...\x1b[0m'];
                    wsReconnectTimeout = setTimeout(() => {
                        connectWebSocket();
                    }, 3000);
                } else if (container?.status?.toLowerCase() !== 'running') {
                    logs = [...logs, '\x1b[31m● Container stopped\x1b[0m'];
                    stats = null; // Clear stats when stopped
                }
            });
        };
    }

    async function startContainer() {
        // Check if already running
        if (isRunning) {
            toast.error('Server is already running');
            return;
        }

        actionLoading = 'start';
        try {
            await api.startContainer(containerId);
            await loadContainer();
            // Close existing connections before creating new ones
            if (ws) {
                ws.onclose = null; // Prevent reconnect logic
                ws.close();
                ws = null;
            }
            if (statsWs) {
                statsWs.onclose = null;
                statsWs.close();
                statsWs = null;
            }
            // Clear any pending reconnect timeouts
            if (wsReconnectTimeout) {
                clearTimeout(wsReconnectTimeout);
                wsReconnectTimeout = null;
            }
            if (statsReconnectTimeout) {
                clearTimeout(statsReconnectTimeout);
                statsReconnectTimeout = null;
            }
            // Connect fresh
            connectWebSocket();
            setTimeout(() => connectStatsWebSocket(), 2000); // Give container time to start
            toast.success('Server started');
        } catch (e: any) {
            toast.error(e.message || 'Failed to start server');
        } finally {
            actionLoading = '';
        }
    }

    async function stopContainer() {
        actionLoading = 'stop';
        try {
            // Close stats WebSocket as it won't work during shutdown
            if (statsWs) {
                statsWs.onclose = null;
                statsWs.close();
                statsWs = null;
            }
            // Use graceful stop - docker stop sends SIGTERM to the container
            // The server will receive the signal and shut down gracefully (show saving logs, etc.)
            // The container has 30 seconds to stop before being force killed
            await api.gracefulStop(containerId, 30);
            // Don't reload container immediately - let the logs show the shutdown
            // The WebSocket onclose handler will update the status when container stops
            toast.success('Stopping server...');
        } catch (e: any) {
            toast.error(e.message || 'Failed to stop server');
        } finally {
            actionLoading = '';
        }
    }

    async function restartContainer() {
        actionLoading = 'restart';
        try {
            if (statsWs) {
                statsWs.onclose = null;
                statsWs.close();
                statsWs = null;
            }
            // Clear any pending reconnect timeouts
            if (wsReconnectTimeout) {
                clearTimeout(wsReconnectTimeout);
                wsReconnectTimeout = null;
            }
            if (statsReconnectTimeout) {
                clearTimeout(statsReconnectTimeout);
                statsReconnectTimeout = null;
            }

            await api.restartContainer(containerId);

            // The WebSocket will disconnect when the container stops
            // The onclose handler will try to reconnect automatically
            // We just need to make sure it reconnects after the container restarts

            toast.success('Server restarting...');
        } catch (e: any) {
            toast.error(e.message || 'Failed to restart server');
        } finally {
            actionLoading = '';
        }
    }

    async function killContainer() {
        actionLoading = 'kill';
        try {
            await api.killContainer(containerId);
            showKillModal = false;
            await loadContainer();
            toast.success('Server killed');
        } catch (e: any) {
            toast.error(e.message || 'Failed to kill server');
        } finally {
            actionLoading = '';
        }
    }

    async function setSftpPassword() {
        if (sftpPassword !== sftpPasswordConfirm) {
            toast.error('Passwords do not match');
            return;
        }
        if (sftpPassword.length < 8) {
            toast.error('Password must be at least 8 characters');
            return;
        }

        settingSftpPassword = true;
        try {
            await api.setSftpPassword(containerId, sftpPassword);
            sftpPassword = '';
            sftpPasswordConfirm = '';
            await loadContainer();
            toast.success('SFTP password set successfully');
        } catch (e: any) {
            toast.error(e.message || 'Failed to set SFTP password');
        } finally {
            settingSftpPassword = false;
        }
    }

    async function saveSettings() {
        savingSettings = true;
        try {
            await api.updateContainer(containerId, {
                memoryLimit: editMemory,
                serverMemory: editServerMemory,
                cpuLimit: editCpu,
                diskLimit: editDisk,
                swapLimit: editSwap,
            });
            await loadContainer();
            toast.success('Settings saved. Restart the server to apply changes.');
        } catch (e: any) {
            toast.error(e.message || 'Failed to save settings');
        } finally {
            savingSettings = false;
        }
    }

    async function loadAllocations() {
        loadingAllocations = true;
        try {
            containerAllocations = await api.getContainerAllocations(containerId);
        } catch (e: any) {
            console.error('Failed to load allocations:', e);
        } finally {
            loadingAllocations = false;
        }
    }

    async function loadAvailableAllocations() {
        try {
            availableAllocations = await api.getAvailableAllocations(containerId);
        } catch (e: any) {
            console.error('Failed to load available allocations:', e);
        }
    }

    async function openAddAllocationModal() {
        await loadAvailableAllocations();
        showAddAllocationModal = true;
    }

    async function addAllocation(allocationId: string) {
        addingAllocation = true;
        try {
            await api.addAllocation(containerId, allocationId);
            await loadContainer();
            await loadAllocations();
            showAddAllocationModal = false;
            toast.success('Allocation added');
        } catch (e: any) {
            toast.error(e.message || 'Failed to add allocation');
        } finally {
            addingAllocation = false;
        }
    }

    async function removeAllocation(allocationId: string) {
        if (!confirm('Are you sure you want to remove this allocation?')) return;
        try {
            await api.removeAllocation(containerId, allocationId);
            await loadContainer();
            await loadAllocations();
            toast.success('Allocation removed');
        } catch (e: any) {
            toast.error(e.message || 'Failed to remove allocation');
        }
    }

    async function setAsPrimary(allocationId: string) {
        settingPrimary = allocationId;
        try {
            await api.assignAllocation(containerId, allocationId);
            await loadContainer();
            await loadAllocations();
            toast.success('Primary allocation updated');
        } catch (e: any) {
            toast.error(e.message || 'Failed to set primary allocation');
        } finally {
            settingPrimary = '';
        }
    }

    function sendCommand() {
        if (!isRunning) {
            toast.error('Container is not running');
            return;
        }
        if (!ws || ws.readyState !== WebSocket.OPEN) {
            toast.error('Console not connected');
            return;
        }
        if (command.trim()) {
            // Send command as plain text to be forwarded to container stdin
            ws.send(command.trim());
            logs = [...logs, `\x1b[36m> ${command}\x1b[0m`];
            command = '';
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Enter') {
            sendCommand();
        }
    }

    async function copyToClipboard(text: string, field: string) {
        await navigator.clipboard.writeText(text);
        copiedField = field;
        toast.success('Copied to clipboard');
        setTimeout(() => copiedField = null, 2000);
    }

    async function loadFiles(path: string = '/') {
        loadingFiles = true;
        try {
            const response = await api.listFiles(containerId, path);
            files = response as FileEntry[];
            currentPath = path;
            selectedFile = null;
            fileContent = '';
            editingFile = false;
        } catch (e: any) {
            toast.error(e.message || 'Failed to load files');
        } finally {
            loadingFiles = false;
        }
    }

    async function openFile(file: FileEntry) {
        if (file.isDir) {
            const newPath = currentPath === '/' ? `/${file.name}` : `${currentPath}/${file.name}`;
            await loadFiles(newPath);
        } else {
            try {
                const content = await api.readFile(containerId, `${currentPath}/${file.name}`);
                selectedFile = file;
                fileContent = content;
                editingFile = true;
            } catch (e: any) {
                toast.error(e.message || 'Failed to read file');
            }
        }
    }

    async function saveFile() {
        if (!selectedFile) return;
        savingFile = true;
        try {
            await api.writeFile(containerId, `${currentPath}/${selectedFile.name}`, fileContent);
            toast.success('File saved');
        } catch (e: any) {
            toast.error(e.message || 'Failed to save file');
        } finally {
            savingFile = false;
        }
    }

    function goUp() {
        const parts = currentPath.split('/').filter(Boolean);
        parts.pop();
        loadFiles('/' + parts.join('/'));
    }

    function formatBytes(bytes: number): string {
        if (bytes === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
    }

    function formatStatsMemory(bytes: number): string {
        if (bytes === 0) return '0 MB';
        const mb = bytes / (1024 * 1024);
        if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
        return `${mb.toFixed(0)} MB`;
    }

    function formatMemory(mb: number | null | undefined): string {
        if (!mb) return 'Unlimited';
        if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
        return `${mb} MB`;
    }

    function getStatusColor(status: string) {
        switch (status?.toLowerCase()) {
            case 'running': return 'text-emerald-400';
            case 'stopped': return 'text-red-400';
            default: return 'text-yellow-400';
        }
    }

    function formatAnsiColors(text: string): string {
        const colors: Record<string, string> = {
            '0': '</span>',
            '31': '<span class="text-red-400">',
            '32': '<span class="text-emerald-400">',
            '33': '<span class="text-yellow-400">',
            '34': '<span class="text-blue-400">',
            '35': '<span class="text-purple-400">',
            '36': '<span class="text-cyan-400">',
        };
        return text.replace(/\x1b\[(\d+)m/g, (_, code) => colors[code] || '');
    }

    function switchTab(tab: typeof activeTab) {
        activeTab = tab;
        if (tab === 'files' && files.length === 0) {
            loadFiles('/');
        }
        if (tab === 'settings' && containerAllocations.length === 0) {
            loadAllocations();
        }
    }
</script>

<div class="h-[calc(100vh-4rem)] flex flex-col">
    {#if loading}
        <div class="flex items-center justify-center flex-1">
            <div class="text-center">
                <div class="spinner w-8 h-8 mx-auto mb-4"></div>
                <p class="text-dark-400">Loading server...</p>
            </div>
        </div>
    {:else if container}
        <div class="flex-shrink-0 border-b border-dark-700/50 bg-dark-900/50 backdrop-blur-sm">
            <div class="px-6 py-4">
                <div class="flex items-center justify-between">
                    <div class="flex items-center gap-4 min-w-0">
                        <a href="/containers" class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-800 transition-colors flex-shrink-0">
                            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" />
                            </svg>
                        </a>
                        <div class="min-w-0">
                            <h1 class="text-xl font-bold text-white truncate max-w-[300px]">{container.name}</h1>
                            <div class="flex items-center gap-2 mt-1 flex-wrap">
                                {#if container.allocationIp && container.allocationPort}
                                    <code class="text-sm font-mono text-primary-400 bg-dark-800 px-2 py-0.5 rounded">{container.allocationIp}:{container.allocationPort}</code>
                                    <span class="text-dark-600 flex-shrink-0">•</span>
                                {/if}
                                <span class={`text-sm font-medium flex-shrink-0 ${getStatusColor(container.status)}`}>
                                    {container.status === 'running' ? '● Online' : '○ Offline'}
                                </span>
                            </div>
                        </div>
                    </div>
                    <!-- Stats Display -->
                    {#if isRunning && stats}
                        <div class="hidden md:flex items-center gap-4 px-4">
                            <div class="text-center">
                                <div class="text-xs text-dark-400 mb-1">CPU</div>
                                <div class="text-sm font-medium text-white">{stats.cpuPercent.toFixed(1)}%</div>
                                <div class="w-16 h-1 bg-dark-700 rounded-full mt-1 overflow-hidden">
                                    <div class="h-full bg-primary-500 rounded-full transition-all" style="width: {Math.min(stats.cpuPercent, 100)}%"></div>
                                </div>
                            </div>
                            <div class="text-center">
                                <div class="text-xs text-dark-400 mb-1">Memory</div>
                                <div class="text-sm font-medium text-white">{formatStatsMemory(stats.memoryUsage)}</div>
                                <div class="w-16 h-1 bg-dark-700 rounded-full mt-1 overflow-hidden">
                                    <div class="h-full bg-emerald-500 rounded-full transition-all" style="width: {Math.min(stats.memoryPercent, 100)}%"></div>
                                </div>
                            </div>
                            <div class="text-center">
                                <div class="text-xs text-dark-400 mb-1">Network</div>
                                <div class="text-xs font-medium text-white">↓{formatBytes(stats.networkRx)}</div>
                                <div class="text-xs font-medium text-white">↑{formatBytes(stats.networkTx)}</div>
                            </div>
                        </div>
                    {/if}
                    <div class="flex items-center gap-2 flex-shrink-0">
                        {#if isRunning}
                            <button on:click={restartContainer} disabled={!!actionLoading} class="btn-secondary">
                                {#if actionLoading === 'restart'}<span class="spinner w-4 h-4"></span>{/if}
                                Restart
                            </button>
                            <button on:click={stopContainer} disabled={!!actionLoading} class="btn-danger">
                                {#if actionLoading === 'stop'}<span class="spinner w-4 h-4"></span>{/if}
                                Stop
                            </button>
                        {:else}
                            <button on:click={startContainer} disabled={!!actionLoading} class="btn-success">
                                {#if actionLoading === 'start'}<span class="spinner w-4 h-4"></span>{/if}
                                Start
                            </button>
                        {/if}
                    </div>
                </div>
                <div class="flex gap-1 mt-4 -mb-4">
                    <button on:click={() => switchTab('console')} class="px-4 py-2 rounded-t-lg text-sm font-medium transition-colors {activeTab === 'console' ? 'bg-dark-800 text-white' : 'text-dark-400 hover:text-white hover:bg-dark-800/50'}">
                        Console
                    </button>
                    <button on:click={() => switchTab('files')} class="px-4 py-2 rounded-t-lg text-sm font-medium transition-colors {activeTab === 'files' ? 'bg-dark-800 text-white' : 'text-dark-400 hover:text-white hover:bg-dark-800/50'}">
                        Files
                    </button>
                    <button on:click={() => switchTab('sftp')} class="px-4 py-2 rounded-t-lg text-sm font-medium transition-colors {activeTab === 'sftp' ? 'bg-dark-800 text-white' : 'text-dark-400 hover:text-white hover:bg-dark-800/50'}">
                        FTP
                    </button>
                    <button on:click={() => switchTab('settings')} class="px-4 py-2 rounded-t-lg text-sm font-medium transition-colors {activeTab === 'settings' ? 'bg-dark-800 text-white' : 'text-dark-400 hover:text-white hover:bg-dark-800/50'}">
                        Settings
                    </button>
                </div>
            </div>
        </div>

        <div class="flex-1 overflow-hidden bg-dark-800">
            {#if activeTab === 'console'}
                <div class="h-full flex flex-col">
                    <div bind:this={logsContainer} class="flex-1 overflow-y-auto p-4 font-mono text-sm bg-dark-950">
                        {#each logs as log}
                            <div class="whitespace-pre-wrap break-all">{@html formatAnsiColors(log)}</div>
                        {/each}
                    </div>
                    <div class="flex-shrink-0 p-4 border-t border-dark-700 bg-dark-900">
                        {#if isRunning}
                            <div class="flex gap-2">
                                <span class="text-primary-400 font-mono">$</span>
                                <input type="text" bind:value={command} on:keydown={handleKeydown} placeholder="Type a command..." class="flex-1 bg-transparent text-white font-mono text-sm focus:outline-none placeholder-dark-500" />
                            </div>
                        {:else}
                            <div class="text-center text-dark-500 text-sm py-1">
                                Container is not running. Start the server to use the console.
                            </div>
                        {/if}
                    </div>
                </div>
            {:else if activeTab === 'files'}
                <div class="h-full flex">
                    <div class="w-80 border-r border-dark-700 flex flex-col">
                        <div class="p-3 border-b border-dark-700 flex items-center gap-2">
                            {#if currentPath !== '/'}
                                <button on:click={goUp} class="p-1.5 rounded hover:bg-dark-700 text-dark-400 hover:text-white">
                                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" />
                                    </svg>
                                </button>
                            {/if}
                            <span class="text-sm font-mono text-dark-400 truncate">{currentPath}</span>
                        </div>
                        <div class="flex-1 overflow-y-auto">
                            {#if loadingFiles}
                                <div class="p-4 text-center text-dark-400"><div class="spinner w-5 h-5 mx-auto"></div></div>
                            {:else if files.length === 0}
                                <div class="p-4 text-center text-dark-500 text-sm">Empty folder</div>
                            {:else}
                                {#each files as file}
                                    <button on:click={() => openFile(file)} class="w-full px-3 py-2 flex items-center gap-3 hover:bg-dark-700/50 text-left {selectedFile?.name === file.name ? 'bg-dark-700' : ''}">
                                        {#if file.isDir}
                                            <svg class="w-5 h-5 text-yellow-400 flex-shrink-0" fill="currentColor" viewBox="0 0 24 24"><path d="M10 4H4a2 2 0 00-2 2v12a2 2 0 002 2h16a2 2 0 002-2V8a2 2 0 00-2-2h-8l-2-2z" /></svg>
                                        {:else}
                                            <svg class="w-5 h-5 text-dark-400 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m2.25 0H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z" /></svg>
                                        {/if}
                                        <div class="min-w-0 flex-1">
                                            <p class="text-sm text-white truncate">{file.name}</p>
                                            {#if !file.isDir}<p class="text-xs text-dark-500">{formatBytes(file.size)}</p>{/if}
                                        </div>
                                    </button>
                                {/each}
                            {/if}
                        </div>
                    </div>
                    <div class="flex-1 flex flex-col">
                        {#if editingFile && selectedFile}
                            <div class="p-3 border-b border-dark-700 flex items-center justify-between">
                                <span class="text-sm font-medium text-white">{selectedFile.name}</span>
                                <button on:click={saveFile} disabled={savingFile} class="btn-success text-sm">
                                    {#if savingFile}<span class="spinner w-4 h-4"></span>{/if}
                                    Save
                                </button>
                            </div>
                            <textarea bind:value={fileContent} class="flex-1 w-full p-4 bg-dark-950 text-white font-mono text-sm resize-none focus:outline-none" spellcheck="false"></textarea>
                        {:else}
                            <div class="flex-1 flex items-center justify-center text-dark-500">Select a file to edit</div>
                        {/if}
                    </div>
                </div>
            {:else if activeTab === 'sftp'}
                <div class="p-6 max-w-2xl">
                    <h2 class="text-lg font-semibold text-white mb-4">FTP Access</h2>
                    <div class="space-y-4">
                        <div class="card p-4">
                            <h3 class="text-sm font-medium text-dark-400 mb-3">Connection Details</h3>
                            <div class="space-y-3">
                                <div class="flex items-center justify-between">
                                    <span class="text-dark-400">Host</span>
                                    <div class="flex items-center gap-2">
                                        <span class="font-mono text-white">localhost</span>
                                        <button on:click={() => copyToClipboard('localhost', 'host')} class="text-dark-400 hover:text-white transition-colors">
                                            {#if copiedField === 'host'}
                                                <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
                                            {:else}
                                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M15.666 3.888A2.25 2.25 0 0013.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 01-.75.75H9.75a.75.75 0 01-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 01-2.25 2.25H6.75A2.25 2.25 0 014.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 011.927-.184" /></svg>
                                            {/if}
                                        </button>
                                    </div>
                                </div>
                                <div class="flex items-center justify-between">
                                    <span class="text-dark-400">Port</span>
                                    <div class="flex items-center gap-2">
                                        <span class="font-mono text-white">2121</span>
                                        <button on:click={() => copyToClipboard('2121', 'port')} class="text-dark-400 hover:text-white transition-colors">
                                            {#if copiedField === 'port'}
                                                <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
                                            {:else}
                                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M15.666 3.888A2.25 2.25 0 0013.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 01-.75.75H9.75a.75.75 0 01-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 01-2.25 2.25H6.75A2.25 2.25 0 014.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 011.927-.184" /></svg>
                                            {/if}
                                        </button>
                                    </div>
                                </div>
                                <div class="flex items-center justify-between">
                                    <span class="text-dark-400">Username</span>
                                    <div class="flex items-center gap-2">
                                        <span class="font-mono text-white">{container?.sftpUser || container?.id.substring(0, 8)}</span>
                                        <button on:click={() => copyToClipboard(container?.sftpUser || container?.id.substring(0, 8) || '', 'user')} class="text-dark-400 hover:text-white transition-colors">
                                            {#if copiedField === 'user'}
                                                <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
                                            {:else}
                                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M15.666 3.888A2.25 2.25 0 0013.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 01-.75.75H9.75a.75.75 0 01-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 01-2.25 2.25H6.75A2.25 2.25 0 014.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 011.927-.184" /></svg>
                                            {/if}
                                        </button>
                                    </div>
                                </div>
                                <div class="flex items-center justify-between">
                                    <span class="text-dark-400">Password</span>
                                    <span class="text-white">{hasSftpPassword ? '••••••••' : 'Not set'}</span>
                                </div>
                            </div>
                        </div>

                        <div class="card p-4">
                            <h3 class="text-sm font-medium text-dark-400 mb-3">{hasSftpPassword ? 'Change' : 'Set'} FTP Password</h3>
                            <form on:submit|preventDefault={setSftpPassword} class="space-y-3">
                                <input type="password" bind:value={sftpPassword} placeholder="New password" class="input w-full" minlength="8" required />
                                <input type="password" bind:value={sftpPasswordConfirm} placeholder="Confirm password" class="input w-full" minlength="8" required />
                                <button type="submit" class="btn-primary w-full" disabled={settingSftpPassword}>
                                    {#if settingSftpPassword}<span class="spinner w-4 h-4"></span>{/if}
                                    {hasSftpPassword ? 'Update Password' : 'Set Password'}
                                </button>
                            </form>
                        </div>
                    </div>
                </div>
            {:else if activeTab === 'settings'}
                <div class="p-6 max-w-2xl overflow-y-auto h-full">
                    <h2 class="text-lg font-semibold text-white mb-4">Server Settings</h2>
                    <div class="space-y-4">
                        <!-- Server Information -->
                        <div class="card p-4">
                            <h3 class="text-sm font-medium text-dark-400 mb-3">Server Information</h3>
                            <div class="grid grid-cols-2 gap-4">
                                <div><span class="text-dark-400 text-sm">Name</span><p class="text-white font-medium truncate">{container.name}</p></div>
                                <div><span class="text-dark-400 text-sm">Image</span><p class="text-white font-mono text-sm truncate">{container.image}</p></div>
                                <div><span class="text-dark-400 text-sm">Container ID</span><p class="text-white font-mono text-xs truncate">{container.id}</p></div>
                                <div><span class="text-dark-400 text-sm">Created</span><p class="text-white">{new Date(container.createdAt).toLocaleDateString()}</p></div>
                            </div>
                        </div>

                        <!-- Network / Allocation Info -->
                        <div class="card p-4">
                            <div class="flex items-center justify-between mb-3">
                                <h3 class="text-sm font-medium text-dark-400">Network Allocations</h3>
                                <button on:click={openAddAllocationModal} class="btn-secondary text-xs py-1 px-2">
                                    <svg class="w-3 h-3 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4" /></svg>
                                    Add Allocation
                                </button>
                            </div>

                            {#if loadingAllocations}
                                <div class="text-center py-4"><span class="spinner w-5 h-5"></span></div>
                            {:else if containerAllocations.length === 0}
                                <p class="text-dark-500 text-sm">No allocations assigned</p>
                            {:else}
                                <div class="space-y-2">
                                    {#each containerAllocations as alloc}
                                        <div class="flex items-center justify-between bg-dark-900/50 rounded-lg p-3">
                                            <div class="flex items-center gap-3">
                                                <code class="text-primary-400 font-mono text-sm">{alloc.ip}:{alloc.port}</code>
                                                <span class="text-xs text-dark-500 font-mono">{alloc.protocol}</span>
                                                {#if alloc.isPrimary}
                                                    <span class="text-xs bg-primary-500/20 text-primary-400 px-2 py-0.5 rounded">Primary</span>
                                                {/if}
                                            </div>
                                            <div class="flex items-center gap-2">
                                                {#if !alloc.isPrimary}
                                                    <button
                                                        on:click={() => alloc.allocationId && setAsPrimary(alloc.allocationId)}
                                                        disabled={settingPrimary === alloc.allocationId}
                                                        class="text-xs text-dark-400 hover:text-primary-400 transition-colors px-2 py-1 rounded hover:bg-dark-700"
                                                        title="Set as primary"
                                                    >
                                                        {#if settingPrimary === alloc.allocationId}
                                                            <span class="spinner w-3 h-3"></span>
                                                        {:else}
                                                            Set Primary
                                                        {/if}
                                                    </button>
                                                {/if}
                                                <button on:click={() => copyToClipboard(`${alloc.ip}:${alloc.port}`, `alloc-${alloc.id}`)} class="text-dark-400 hover:text-white">
                                                    {#if copiedField === `alloc-${alloc.id}`}
                                                        <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
                                                    {:else}
                                                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" /></svg>
                                                    {/if}
                                                </button>
                                                <button
                                                    on:click={() => alloc.allocationId && removeAllocation(alloc.allocationId)}
                                                    class="text-dark-400 hover:text-red-400"
                                                    title={alloc.isPrimary ? "Remove primary allocation (another will be promoted)" : "Remove allocation"}
                                                >
                                                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
                                                </button>
                                            </div>
                                        </div>
                                    {/each}
                                </div>
                            {/if}
                        </div>

                        <!-- Resource Limits (Editable) -->
                        <div class="card p-4">
                            <h3 class="text-sm font-medium text-dark-400 mb-3">Resource Limits</h3>
                            <form on:submit|preventDefault={saveSettings} class="space-y-4">
                                <div class="grid grid-cols-2 gap-4">
                                    <div>
                                        <label class="text-dark-400 text-sm block mb-1">Server Memory (MB)</label>
                                        <input type="number" bind:value={editServerMemory} min="128" class="input w-full" placeholder="e.g., 1024" />
                                        <p class="text-dark-500 text-xs mt-1">JVM heap memory (-Xmx)</p>
                                    </div>
                                    <div>
                                        <label class="text-dark-400 text-sm block mb-1">Container Memory (MB)</label>
                                        <input type="number" bind:value={editMemory} min="128" class="input w-full" placeholder="e.g., 1280" />
                                        <p class="text-dark-500 text-xs mt-1">Docker limit (should be ~20% higher)</p>
                                    </div>
                                    <div>
                                        <label class="text-dark-400 text-sm block mb-1">CPU Limit (cores)</label>
                                        <input type="number" bind:value={editCpu} min="0.1" step="0.1" class="input w-full" placeholder="e.g., 1.0" />
                                    </div>
                                    <div>
                                        <label class="text-dark-400 text-sm block mb-1">Disk Space (MB)</label>
                                        <input type="number" bind:value={editDisk} min="1024" class="input w-full" placeholder="e.g., 5120" />
                                    </div>
                                    <div>
                                        <label class="text-dark-400 text-sm block mb-1">Swap (MB)</label>
                                        <input type="number" bind:value={editSwap} min="0" class="input w-full" placeholder="e.g., 0" />
                                    </div>
                                </div>
                                <div class="flex justify-end">
                                    <button type="submit" class="btn-primary" disabled={savingSettings}>
                                        {#if savingSettings}<span class="spinner w-4 h-4 mr-2"></span>{/if}
                                        Save Changes
                                    </button>
                                </div>
                            </form>
                            <p class="text-dark-500 text-xs mt-3">Note: Resource changes take effect after restarting the server.</p>
                        </div>

                        <!-- Danger Zone -->
                        <div class="card p-4 border-red-500/20">
                            <h3 class="text-sm font-medium text-red-400 mb-3">Danger Zone</h3>
                            <p class="text-dark-400 text-sm mb-3">Force kill the server if it's unresponsive.</p>
                            <button on:click={() => showKillModal = true} class="btn-danger">Kill Server</button>
                        </div>
                    </div>
                </div>
            {/if}
        </div>
    {:else}
        <div class="flex items-center justify-center flex-1"><p class="text-dark-400">Server not found</p></div>
    {/if}
</div>

{#if showKillModal}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
        <div class="absolute inset-0 bg-dark-950/80 backdrop-blur-sm" on:click={() => showKillModal = false} role="button" tabindex="-1" on:keydown={(e) => e.key === 'Escape' && (showKillModal = false)}></div>
        <div class="relative card p-6 max-w-md w-full">
            <h2 class="text-xl font-semibold text-white mb-2">Kill Server?</h2>
            <p class="text-dark-400 mb-4">This will forcefully terminate the server. Any unsaved data may be lost.</p>
            <div class="flex gap-3">
                <button on:click={killContainer} disabled={actionLoading === 'kill'} class="btn-danger flex-1">
                    {#if actionLoading === 'kill'}<span class="spinner w-4 h-4"></span>{/if}
                    Kill Server
                </button>
                <button on:click={() => showKillModal = false} class="btn-secondary">Cancel</button>
            </div>
        </div>
    </div>
{/if}

{#if showAddAllocationModal}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
        <div class="absolute inset-0 bg-dark-950/80 backdrop-blur-sm" on:click={() => showAddAllocationModal = false} role="button" tabindex="-1" on:keydown={(e) => e.key === 'Escape' && (showAddAllocationModal = false)}></div>
        <div class="relative card p-6 max-w-md w-full">
            <h2 class="text-xl font-semibold text-white mb-4">Add Allocation</h2>
            {#if availableAllocations.length === 0}
                <p class="text-dark-400 text-sm mb-4">No available allocations for this daemon.</p>
            {:else}
                <p class="text-dark-400 text-sm mb-4">Select an allocation to add to this server:</p>
                <div class="space-y-2 max-h-60 overflow-y-auto mb-4">
                    {#each availableAllocations as alloc}
                        <button
                            on:click={() => addAllocation(alloc.id)}
                            disabled={addingAllocation}
                            class="w-full flex items-center justify-between bg-dark-900/50 hover:bg-dark-700 rounded-lg p-3 transition-colors"
                        >
                            <code class="text-primary-400 font-mono">{alloc.ip}:{alloc.port}</code>
                            {#if addingAllocation}
                                <span class="spinner w-4 h-4"></span>
                            {:else}
                                <svg class="w-4 h-4 text-dark-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4" /></svg>
                            {/if}
                        </button>
                    {/each}
                </div>
            {/if}
            <button on:click={() => showAddAllocationModal = false} class="btn-secondary w-full">Close</button>
        </div>
    </div>
{/if}

