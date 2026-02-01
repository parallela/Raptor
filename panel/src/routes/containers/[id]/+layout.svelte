<script lang="ts">
    import { onMount, onDestroy, setContext } from 'svelte';
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import { writable, type Writable } from 'svelte/store';
    import { api, createWebSocket, createStatsWebSocket } from '$lib/api';
    import { user } from '$lib/stores';
    import toast from 'svelte-french-toast';
    import type { Container, ContainerPort, ContainerAllocation, Allocation } from '$lib/types';

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

    const containerStore: Writable<Container | null> = writable(null);
    const portsStore: Writable<ContainerPort[]> = writable([]);
    const statsStore: Writable<ContainerStats | null> = writable(null);
    const logsStore: Writable<string[]> = writable([]);
    const loadingStore: Writable<boolean> = writable(true);
    const actionLoadingStore: Writable<string> = writable('');
    const allocationsStore: Writable<ContainerAllocation[]> = writable([]);
    const availableAllocationsStore: Writable<Allocation[]> = writable([]);

    let ws: WebSocket | null = null;
    let statsWs: WebSocket | null = null;
    let statusInterval: ReturnType<typeof setInterval> | null = null;
    let wsReconnectTimeout: ReturnType<typeof setTimeout> | null = null;
    let statsReconnectTimeout: ReturnType<typeof setTimeout> | null = null;

    let intentionalStop = false;

    let isPageVisible = true;
    let hasShownConnectedMessage = false;

    setContext('container', containerStore);
    setContext('ports', portsStore);
    setContext('stats', statsStore);
    setContext('logs', logsStore);
    setContext('loading', loadingStore);
    setContext('actionLoading', actionLoadingStore);
    setContext('allocations', allocationsStore);
    setContext('availableAllocations', availableAllocationsStore);

    setContext('actions', {
        startContainer,
        stopContainer,
        restartContainer,
        killContainer,
        sendCommand,
        loadContainer,
        loadAllocations
    });

    $: containerId = $page.params.id as string;
    $: container = $containerStore;
    $: isRunning = container?.status?.toLowerCase() === 'running';

    onMount(async () => {
        if (!$user) {
            goto('/login');
            return;
        }
        await loadContainer();
        connectWebSocket();
        connectStatsWebSocket();
        statusInterval = setInterval(async () => {
            if (isPageVisible) {
                await loadContainer();
            }
        }, 10000);

        if (typeof document !== 'undefined') {
            document.addEventListener('visibilitychange', handleVisibilityChange);
        }
    });

    onDestroy(() => {
        if (ws) ws.close();
        if (statsWs) statsWs.close();
        if (statusInterval) clearInterval(statusInterval);
        if (wsReconnectTimeout) clearTimeout(wsReconnectTimeout);
        if (statsReconnectTimeout) clearTimeout(statsReconnectTimeout);
        if (typeof document !== 'undefined') {
            document.removeEventListener('visibilitychange', handleVisibilityChange);
        }
    });

    function handleVisibilityChange() {
        isPageVisible = !document.hidden;

        if (isPageVisible) {
            if ($containerStore?.status?.toLowerCase() === 'running') {
                if (!ws || ws.readyState !== WebSocket.OPEN) {
                    connectWebSocket(false, true);
                }
                if (!statsWs || statsWs.readyState !== WebSocket.OPEN) {
                    connectStatsWebSocket();
                }
            }
            loadContainer();
        }
    }

    async function loadContainer() {
        try {
            const [containerData, portsData] = await Promise.all([
                api.getContainer(containerId),
                api.getContainerPorts(containerId).catch(() => [])
            ]);
            containerStore.set(containerData);
            portsStore.set(portsData);
        } catch (e) {
            console.error(e);
            toast.error('Failed to load server');
        } finally {
            loadingStore.set(false);
        }
    }

    async function loadAllocations() {
        try {
            const [allocations, available] = await Promise.all([
                api.getContainerAllocations(containerId),
                api.getAvailableAllocations(containerId).catch(() => [])
            ]);
            allocationsStore.set(allocations);
            availableAllocationsStore.set(available);
        } catch (e) {
            console.error('Failed to load allocations', e);
        }
    }

    function connectStatsWebSocket() {
        if (statsWs) statsWs.close();
        statsWs = createStatsWebSocket(containerId);
        if (!statsWs) return;

        statsWs.onmessage = (event) => {
            try {
                statsStore.set(JSON.parse(event.data));
            } catch (e) {
                console.debug('Failed to parse stats:', e);
            }
        };

        statsWs.onerror = () => console.debug('Stats WebSocket error');

        statsWs.onclose = () => {
            if (!isPageVisible) return;

            if ($containerStore?.status?.toLowerCase() === 'running') {
                statsReconnectTimeout = setTimeout(connectStatsWebSocket, 5000);
            } else {
                statsStore.set(null);
            }
        };
    }

    function connectWebSocket(clearLogs = false, silent = false) {
        if (ws) {
            ws.onclose = null;
            ws.close();
            ws = null;
        }

        ws = createWebSocket(containerId);
        if (!ws) return;

        ws.onopen = () => {
            if (clearLogs) {
                logsStore.set(['\x1b[32m● Connected to server console\x1b[0m']);
                hasShownConnectedMessage = true;
            } else if (!silent && !hasShownConnectedMessage) {
                logsStore.update(logs => [...logs, '\x1b[32m● Connected to server console\x1b[0m']);
                logsStore.update(logs => [...logs, `\x1b[32m● Connected to container: ${containerId}\x1b[0m`]);
                hasShownConnectedMessage = true;
            }
        };

        ws.onmessage = (event) => {
            const data = event.data;
            if (data.includes('Pulling') || data.includes('Downloading') || data.includes('Extracting')) {
                logsStore.update(logs => [...logs, `\x1b[34m[PULL] ${data}\x1b[0m`]);
            } else {
                logsStore.update(logs => [...logs, data]);
            }
        };

        ws.onerror = () => {
            logsStore.update(logs => [...logs, '\x1b[31m● Connection error\x1b[0m']);
        };

        ws.onclose = () => {
            if (intentionalStop) {
                logsStore.update(logs => [...logs, '\x1b[31m● Container stopped\x1b[0m']);
                statsStore.set(null);
                hasShownConnectedMessage = false;
                return;
            }

            if (!isPageVisible) {
                return;
            }

            loadContainer().then(() => {
                if ($containerStore?.status?.toLowerCase() === 'running' && (!ws || ws.readyState === WebSocket.CLOSED)) {
                    logsStore.update(logs => [...logs, '\x1b[33m● Console connection lost, reconnecting...\x1b[0m']);
                    wsReconnectTimeout = setTimeout(() => connectWebSocket(false, false), 3000);
                } else if ($containerStore?.status?.toLowerCase() !== 'running') {
                    logsStore.update(logs => [...logs, '\x1b[31m● Container stopped\x1b[0m']);
                    statsStore.set(null);
                    hasShownConnectedMessage = false;
                }
            });
        };
    }

    async function startContainer() {
        if (isRunning) {
            toast.error('Server is already running');
            return;
        }
        intentionalStop = false;
        actionLoadingStore.set('start');
        try {
            await api.startContainer(containerId);
            await loadContainer();
            reconnectWebSockets();
        } catch (e: any) {
            toast.error(e.message || 'Failed to start server');
        } finally {
            actionLoadingStore.set('');
        }
    }

    async function stopContainer() {
        intentionalStop = true;
        actionLoadingStore.set('stop');
        try {
            await api.stopContainer(containerId);
            await loadContainer();
        } catch (e: any) {
            toast.error(e.message || 'Failed to stop server');
        } finally {
            actionLoadingStore.set('');
        }
    }

    async function restartContainer() {
        intentionalStop = false;
        actionLoadingStore.set('restart');
        try {
            await api.restartContainer(containerId);
            await loadContainer();
            reconnectWebSockets();
        } catch (e: any) {
            toast.error(e.message || 'Failed to restart server');
        } finally {
            actionLoadingStore.set('');
        }
    }

    async function killContainer() {
        intentionalStop = true;
        actionLoadingStore.set('kill');
        try {
            await api.killContainer(containerId);
            await loadContainer();
        } catch (e: any) {
            toast.error(e.message || 'Failed to kill server');
        } finally {
            actionLoadingStore.set('');
        }
    }

    function sendCommand(command: string) {
        if (ws && ws.readyState === WebSocket.OPEN && command.trim()) {
            ws.send(command);
            logsStore.update(logs => [...logs, command]);
        }
    }

    function reconnectWebSockets() {
        if (ws) {
            ws.onclose = null;
            ws.close();
            ws = null;
        }
        if (statsWs) {
            statsWs.onclose = null;
            statsWs.close();
            statsWs = null;
        }
        if (wsReconnectTimeout) clearTimeout(wsReconnectTimeout);
        if (statsReconnectTimeout) clearTimeout(statsReconnectTimeout);

        hasShownConnectedMessage = false;

        setTimeout(() => {
            connectWebSocket();
            connectStatsWebSocket();
        }, 500);
    }

    function formatBytes(bytes: number): string {
        if (bytes === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
    }

    function formatStatsMemory(bytes: number): string {
        const mb = bytes / (1024 * 1024);
        if (mb >= 1024) return (mb / 1024).toFixed(2) + ' GB';
        return mb.toFixed(0) + ' MB';
    }

    function getStatusColor(status?: string): string {
        switch (status?.toLowerCase()) {
            case 'running': return 'text-emerald-400';
            case 'stopped': case 'exited': return 'text-red-400';
            case 'starting': case 'restarting': return 'text-yellow-400';
            default: return 'text-dark-400';
        }
    }

    $: currentPath = $page.url.pathname;
    $: activeTab = currentPath.includes('/files') ? 'files'
        : currentPath.includes('/ftp') ? 'ftp'
        : currentPath.includes('/settings') ? 'settings'
        : 'console';
</script>

<div class="h-[calc(100vh-4rem)] flex flex-col">
    {#if $loadingStore}
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
                    {#if isRunning && $statsStore}
                        <div class="hidden md:flex items-center gap-4 px-4">
                            <div class="text-center">
                                <div class="text-xs text-dark-400 mb-1">CPU</div>
                                <div class="text-sm font-medium text-white">{$statsStore.cpuPercent.toFixed(1)}%</div>
                                <div class="w-16 h-1 bg-dark-700 rounded-full mt-1 overflow-hidden">
                                    <div class="h-full bg-primary-500 rounded-full transition-all" style="width: {Math.min($statsStore.cpuPercent, 100)}%"></div>
                                </div>
                            </div>
                            <div class="text-center">
                                <div class="text-xs text-dark-400 mb-1">Memory</div>
                                <div class="text-sm font-medium text-white">{formatStatsMemory($statsStore.memoryUsage)}</div>
                                <div class="w-16 h-1 bg-dark-700 rounded-full mt-1 overflow-hidden">
                                    <div class="h-full bg-emerald-500 rounded-full transition-all" style="width: {Math.min($statsStore.memoryPercent, 100)}%"></div>
                                </div>
                            </div>
                            <div class="text-center">
                                <div class="text-xs text-dark-400 mb-1">Network</div>
                                <div class="text-xs font-medium text-white">↓{formatBytes($statsStore.networkRx)}</div>
                                <div class="text-xs font-medium text-white">↑{formatBytes($statsStore.networkTx)}</div>
                            </div>
                        </div>
                    {/if}
                    <div class="flex items-center gap-2 flex-shrink-0">
                        {#if isRunning}
                            <button on:click={restartContainer} disabled={!!$actionLoadingStore} class="btn-secondary">
                                {#if $actionLoadingStore === 'restart'}<span class="spinner w-4 h-4"></span>{/if}
                                Restart
                            </button>
                            <button on:click={stopContainer} disabled={!!$actionLoadingStore} class="btn-danger">
                                {#if $actionLoadingStore === 'stop'}<span class="spinner w-4 h-4"></span>{/if}
                                Stop
                            </button>
                        {:else}
                            <button on:click={startContainer} disabled={!!$actionLoadingStore} class="btn-success">
                                {#if $actionLoadingStore === 'start'}<span class="spinner w-4 h-4"></span>{/if}
                                Start
                            </button>
                        {/if}
                    </div>
                </div>
                <div class="flex gap-1 mt-4 -mb-4">
                    <a href="/containers/{containerId}/console" class="px-4 py-2 rounded-t-lg text-sm font-medium transition-colors {activeTab === 'console' ? 'bg-dark-800 text-white' : 'text-dark-400 hover:text-white hover:bg-dark-800/50'}">
                        Console
                    </a>
                    <a href="/containers/{containerId}/files" class="px-4 py-2 rounded-t-lg text-sm font-medium transition-colors {activeTab === 'files' ? 'bg-dark-800 text-white' : 'text-dark-400 hover:text-white hover:bg-dark-800/50'}">
                        Files
                    </a>
                    <a href="/containers/{containerId}/ftp" class="px-4 py-2 rounded-t-lg text-sm font-medium transition-colors {activeTab === 'ftp' ? 'bg-dark-800 text-white' : 'text-dark-400 hover:text-white hover:bg-dark-800/50'}">
                        FTP
                    </a>
                    <a href="/containers/{containerId}/settings" class="px-4 py-2 rounded-t-lg text-sm font-medium transition-colors {activeTab === 'settings' ? 'bg-dark-800 text-white' : 'text-dark-400 hover:text-white hover:bg-dark-800/50'}">
                        Settings
                    </a>
                </div>
            </div>
        </div>

        <div class="flex-1 overflow-hidden bg-dark-800">
            <slot />
        </div>
    {:else}
        <div class="flex items-center justify-center flex-1">
            <p class="text-dark-400">Server not found</p>
        </div>
    {/if}
</div>
