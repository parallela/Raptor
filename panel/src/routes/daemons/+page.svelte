<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { api } from '$lib/api';
    import { user } from '$lib/stores';
    import { goto } from '$app/navigation';
    import toast from 'svelte-french-toast';
    import type { Daemon } from '$lib/types';

    interface DaemonStatus {
        status: string;
        system?: {
            totalMemory: number;
            availableMemory: number;
            cpuCores: number;
            cpuUsage: number;
            totalDisk: number;
            availableDisk: number;
            hostname: string;
        };
    }

    let daemons: Daemon[] = [];
    let daemonStatuses: Record<string, DaemonStatus> = {};
    let loading = true;
    let showCreate = false;
    let showEdit = false;
    let creating = false;
    let editing = false;
    let copiedKey: string | null = null;
    let deleteModal: { show: boolean; daemon: Daemon | null } = { show: false, daemon: null };
    let deleting = false;
    let editingDaemon: Daemon | null = null;
    let statsSocket: WebSocket | null = null;

    let newDaemon = {
        name: '',
        host: '',
        port: 8080,
        location: ''
    };

    let editDaemon = {
        name: '',
        host: '',
        port: 8080,
        location: ''
    };

    onMount(async () => {
        if (!$user) {
            goto('/login');
            return;
        }
        await loadData();
        connectStatsWebSocket();
    });

    onDestroy(() => {
        if (statsSocket) {
            statsSocket.close();
        }
    });

    function connectStatsWebSocket() {
        const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const apiHost = import.meta.env.VITE_API_URL?.replace(/^https?:\/\//, '') || 'localhost:3000';
        const wsUrl = `${wsProtocol}//${apiHost}/ws/daemons/stats`;

        statsSocket = new WebSocket(wsUrl);

        statsSocket.onopen = () => {
            console.log('Connected to daemon stats WebSocket');
        };

        statsSocket.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                if (data.daemonId && data.system) {
                    daemonStatuses = {
                        ...daemonStatuses,
                        [data.daemonId]: {
                            status: 'online',
                            system: data.system
                        }
                    };
                }
            } catch (e) {
                console.error('Failed to parse WebSocket message:', e);
            }
        };

        statsSocket.onerror = (error) => {
            console.error('WebSocket error:', error);
        };

        statsSocket.onclose = () => {
            console.log('WebSocket closed, reconnecting in 5s...');
            setTimeout(() => {
                if (!statsSocket || statsSocket.readyState === WebSocket.CLOSED) {
                    connectStatsWebSocket();
                }
            }, 5000);
        };
    }

    async function loadData() {
        loading = true;
        try {
            daemons = await api.listDaemons();
            fetchAllStatuses();
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    }

    async function fetchAllStatuses() {
        for (const daemon of daemons) {
            fetchDaemonStatus(daemon.id);
        }
    }

    async function fetchDaemonStatus(id: string) {
        try {
            const result = await api.getDaemonStatus(id);
            daemonStatuses = { ...daemonStatuses, [id]: result };
        } catch (e) {
            daemonStatuses = { ...daemonStatuses, [id]: { status: 'offline' } };
        }
    }


    function formatBytes(bytes: number): string {
        if (bytes === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
    }

    function getStatusBadge(status: string): string {
        switch (status) {
            case 'online': return 'badge-success';
            case 'offline': return 'badge-danger';
            case 'checking': return 'badge-neutral';
            default: return 'badge-neutral';
        }
    }

    async function createDaemon() {
        creating = true;
        try {
            await api.createDaemon({
                name: newDaemon.name,
                host: newDaemon.host,
                port: newDaemon.port,
                location: newDaemon.location || undefined
            });
            showCreate = false;
            newDaemon = { name: '', host: '', port: 8080, location: '' };
            await loadData();
            toast.success('Daemon created successfully');
        } catch (e: any) {
            toast.error(e.message || 'Failed to create daemon');
        } finally {
            creating = false;
        }
    }

    function openDeleteModal(daemon: Daemon) {
        deleteModal = { show: true, daemon };
    }

    function closeDeleteModal() {
        deleteModal = { show: false, daemon: null };
    }

    async function confirmDelete() {
        if (!deleteModal.daemon) return;
        deleting = true;
        try {
            await api.deleteDaemon(deleteModal.daemon.id);
            closeDeleteModal();
            await loadData();
            toast.success('Daemon deleted successfully');
        } catch (e: any) {
            toast.error(e.message || 'Failed to delete daemon');
        } finally {
            deleting = false;
        }
    }

    function openEditModal(daemon: Daemon) {
        editingDaemon = daemon;
        editDaemon = {
            name: daemon.name,
            host: daemon.host,
            port: daemon.port,
            location: daemon.location || ''
        };
        showEdit = true;
    }

    function closeEditModal() {
        showEdit = false;
        editingDaemon = null;
    }

    async function updateDaemon() {
        if (!editingDaemon) return;
        editing = true;
        try {
            await api.updateDaemon(editingDaemon.id, {
                name: editDaemon.name,
                host: editDaemon.host,
                port: editDaemon.port,
                location: editDaemon.location || undefined
            });
            closeEditModal();
            await loadData();
            toast.success('Daemon updated successfully');
        } catch (e: any) {
            toast.error(e.message || 'Failed to update daemon');
        } finally {
            editing = false;
        }
    }

    async function copyApiKey(key: string) {
        await navigator.clipboard.writeText(key);
        copiedKey = key;
        toast.success('API key copied to clipboard');
        setTimeout(() => copiedKey = null, 2000);
    }
</script>

<div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
        <div>
            <h1 class="section-title">Daemons</h1>
            <p class="section-subtitle">Manage your server nodes and their configurations</p>
        </div>
        <button on:click={() => showCreate = true} class="btn-primary">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
            Add Daemon
        </button>
    </div>

    <!-- Create Modal -->
    {#if showCreate}
        <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
            <!-- Backdrop -->
            <div
                class="absolute inset-0 bg-dark-950/80 backdrop-blur-sm"
                on:click={() => showCreate = false}
                on:keydown={(e) => e.key === 'Escape' && (showCreate = false)}
                role="button"
                tabindex="-1"
            ></div>

            <!-- Modal -->
            <div class="relative w-full max-w-lg card p-6 animate-slide-up">
                <div class="flex items-center justify-between mb-6">
                    <h2 class="text-xl font-semibold text-white">Add New Daemon</h2>
                    <button
                        on:click={() => showCreate = false}
                        class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-700/50 transition-colors duration-200"
                    >
                        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                </div>

                <form on:submit|preventDefault={createDaemon} class="space-y-5">
                    <div class="input-group">
                        <label for="name" class="input-label">Daemon Name</label>
                        <input type="text" id="name" bind:value={newDaemon.name} class="input" placeholder="e.g. Node 1" required />
                    </div>

                    <div class="grid grid-cols-3 gap-4">
                        <div class="input-group col-span-2">
                            <label for="host" class="input-label">Host</label>
                            <input type="text" id="host" bind:value={newDaemon.host} class="input" placeholder="daemon or IP address" required />
                            <p class="text-xs text-dark-500 mt-1">Use "daemon" for local Docker, or IP for remote</p>
                        </div>
                        <div class="input-group">
                            <label for="port" class="input-label">Port</label>
                            <input type="number" id="port" bind:value={newDaemon.port} class="input" required />
                        </div>
                    </div>

                    <div class="input-group">
                        <label for="location" class="input-label">
                            Location
                            <span class="text-dark-500 font-normal">(optional)</span>
                        </label>
                        <input type="text" id="location" bind:value={newDaemon.location} class="input" placeholder="e.g. US-East" />
                    </div>

                    <div class="flex gap-3 pt-4">
                        <button type="submit" class="btn-success flex-1" disabled={creating}>
                            {#if creating}
                                <span class="spinner"></span>
                                Adding...
                            {:else}
                                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                                </svg>
                                Add Daemon
                            {/if}
                        </button>
                        <button type="button" on:click={() => showCreate = false} class="btn-secondary">
                            Cancel
                        </button>
                    </div>
                </form>
            </div>
        </div>
    {/if}

    {#if loading}
        <div class="flex items-center justify-center py-20">
            <div class="text-center">
                <div class="spinner w-8 h-8 mx-auto mb-4"></div>
                <p class="text-dark-400">Loading daemons...</p>
            </div>
        </div>
    {:else}
        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
            {#each daemons as daemon, i (daemon.id)}
                <div class="card-hover p-6 animate-slide-up" style="animation-delay: {i * 50}ms;">
                    <div class="flex items-start justify-between mb-4">
                        <div class="flex items-center gap-3">
                            <div class="w-11 h-11 rounded-xl bg-gradient-to-br from-primary-500/20 to-primary-600/10 flex items-center justify-center">
                                <svg class="w-5 h-5 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M5.25 14.25h13.5m-13.5 0a3 3 0 01-3-3m3 3a3 3 0 100 6h13.5a3 3 0 100-6m-16.5-3a3 3 0 013-3h13.5a3 3 0 013 3m-19.5 0a4.5 4.5 0 01.9-2.7L5.737 5.1a3.375 3.375 0 012.7-1.35h7.126c1.062 0 2.062.5 2.7 1.35l2.587 3.45a4.5 4.5 0 01.9 2.7m0 0a3 3 0 01-3 3m0 3h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008zm-3 6h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008z" />
                                </svg>
                            </div>
                            <div>
                                <h2 class="text-lg font-semibold text-white">{daemon.name}</h2>
                                {#if daemon.location}
                                    <span class="badge-info text-xs mt-1">{daemon.location}</span>
                                {/if}
                            </div>
                        </div>
                        <div class="flex items-center gap-1">
                            <button
                                on:click={() => openEditModal(daemon)}
                                class="p-2 rounded-lg text-dark-400 hover:text-primary-400 hover:bg-primary-500/10 transition-colors duration-200"
                                title="Edit daemon"
                            >
                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M16.862 4.487l1.687-1.688a1.875 1.875 0 112.652 2.652L10.582 16.07a4.5 4.5 0 01-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 011.13-1.897l8.932-8.931zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0115.75 21H5.25A2.25 2.25 0 013 18.75V8.25A2.25 2.25 0 015.25 6H10" />
                                </svg>
                            </button>
                            <button
                                on:click={() => openDeleteModal(daemon)}
                                class="p-2 rounded-lg text-dark-400 hover:text-red-400 hover:bg-red-500/10 transition-colors duration-200"
                                title="Delete daemon"
                            >
                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0" />
                                </svg>
                            </button>
                        </div>
                    </div>

                    <div class="space-y-3">
                        <div class="flex items-center gap-3 p-3 rounded-lg bg-dark-800/50">
                            <svg class="w-4 h-4 text-dark-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M12 21a9.004 9.004 0 008.716-6.747M12 21a9.004 9.004 0 01-8.716-6.747M12 21c2.485 0 4.5-4.03 4.5-9S14.485 3 12 3m0 18c-2.485 0-4.5-4.03-4.5-9S9.515 3 12 3m0 0a8.997 8.997 0 017.843 4.582M12 3a8.997 8.997 0 00-7.843 4.582m15.686 0A11.953 11.953 0 0112 10.5c-2.998 0-5.74-1.1-7.843-2.918m15.686 0A8.959 8.959 0 0121 12c0 .778-.099 1.533-.284 2.253m0 0A17.919 17.919 0 0112 16.5c-3.162 0-6.133-.815-8.716-2.247m0 0A9.015 9.015 0 013 12c0-1.605.42-3.113 1.157-4.418" />
                            </svg>
                            <div class="flex-1 min-w-0">
                                <p class="text-xs text-dark-400">Host</p>
                                <p class="text-sm font-mono text-white truncate">{daemon.host}:{daemon.port}</p>
                            </div>
                        </div>

                        <div class="flex items-center gap-3 p-3 rounded-lg bg-dark-800/50">
                            <svg class="w-4 h-4 text-dark-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 5.25a3 3 0 013 3m3 0a6 6 0 01-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1121.75 8.25z" />
                            </svg>
                            <div class="flex-1 min-w-0">
                                <p class="text-xs text-dark-400">API Key</p>
                                <div class="flex items-center gap-2">
                                    <p class="text-sm font-mono text-white truncate flex-1">{daemon.apiKey.slice(0, 20)}...</p>
                                    <button
                                        on:click={() => copyApiKey(daemon.apiKey)}
                                        class="p-1 rounded text-dark-400 hover:text-white transition-colors duration-200"
                                        title="Copy API Key"
                                    >
                                        {#if copiedKey === daemon.apiKey}
                                            <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                                <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                                            </svg>
                                        {:else}
                                            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                                <path stroke-linecap="round" stroke-linejoin="round" d="M15.666 3.888A2.25 2.25 0 0013.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 01-.75.75H9a.75.75 0 01-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 01-2.25 2.25H6.75A2.25 2.25 0 014.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 011.927-.184" />
                                            </svg>
                                        {/if}
                                    </button>
                                </div>
                            </div>
                        </div>

                        <!-- System Resources (only shown when online) -->
                        {#if daemonStatuses[daemon.id]?.status === 'online' && daemonStatuses[daemon.id]?.system}
                            {@const sys = daemonStatuses[daemon.id].system}
                            {@const usedMemory = (sys?.totalMemory || 0) - (sys?.availableMemory || 0)}
                            {@const usedDisk = (sys?.totalDisk || 0) - (sys?.availableDisk || 0)}
                            <div class="grid grid-cols-2 gap-2">
                                <div class="p-2 rounded-lg bg-dark-800/50">
                                    <p class="text-xs text-dark-400">CPU</p>
                                    <p class="text-sm font-medium text-white">{sys?.cpuCores} cores @ {sys?.cpuUsage.toFixed(1)}%</p>
                                </div>
                                <div class="p-2 rounded-lg bg-dark-800/50">
                                    <p class="text-xs text-dark-400">Memory</p>
                                    <p class="text-sm font-medium text-white">{formatBytes(usedMemory)} / {formatBytes(sys?.totalMemory || 0)}</p>
                                </div>
                                <div class="p-2 rounded-lg bg-dark-800/50 col-span-2">
                                    <p class="text-xs text-dark-400">Disk</p>
                                    <p class="text-sm font-medium text-white">{formatBytes(usedDisk)} / {formatBytes(sys?.totalDisk || 0)}</p>
                                </div>
                            </div>
                        {/if}
                    </div>

                    <div class="mt-4 pt-4 border-t border-dark-700/50 flex items-center justify-between text-xs text-dark-500">
                        <span>Added {new Date(daemon.createdAt).toLocaleDateString()}</span>
                        <span class={getStatusBadge(daemonStatuses[daemon.id]?.status || 'checking')}>
                            {#if !daemonStatuses[daemon.id] || daemonStatuses[daemon.id]?.status === 'checking'}
                                <span class="spinner w-3 h-3"></span>
                                Checking
                            {:else if daemonStatuses[daemon.id]?.status === 'online'}
                                <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                                Online
                            {:else}
                                <span class="w-1.5 h-1.5 rounded-full bg-red-400"></span>
                                Offline
                            {/if}
                        </span>
                    </div>
                </div>
            {:else}
                <div class="col-span-full card p-12 text-center">
                    <div class="w-16 h-16 rounded-2xl bg-dark-800 flex items-center justify-center mx-auto mb-4">
                        <svg class="w-8 h-8 text-dark-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M5.25 14.25h13.5m-13.5 0a3 3 0 01-3-3m3 3a3 3 0 100 6h13.5a3 3 0 100-6m-16.5-3a3 3 0 013-3h13.5a3 3 0 013 3m-19.5 0a4.5 4.5 0 01.9-2.7L5.737 5.1a3.375 3.375 0 012.7-1.35h7.126c1.062 0 2.062.5 2.7 1.35l2.587 3.45a4.5 4.5 0 01.9 2.7m0 0a3 3 0 01-3 3m0 3h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008zm-3 6h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008z" />
                        </svg>
                    </div>
                    <h3 class="text-lg font-semibold text-white mb-2">No daemons configured</h3>
                    <p class="text-dark-400 mb-6">Add your first daemon to start managing containers</p>
                    <button on:click={() => showCreate = true} class="btn-primary inline-flex">
                        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                        </svg>
                        Add Daemon
                    </button>
                </div>
            {/each}
        </div>
    {/if}

    <!-- Delete Confirmation Modal -->
    {#if deleteModal.show && deleteModal.daemon}
        <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
            <div
                class="absolute inset-0 bg-dark-950/80 backdrop-blur-sm"
                on:click={closeDeleteModal}
                on:keydown={(e) => e.key === 'Escape' && closeDeleteModal()}
                role="button"
                tabindex="-1"
            ></div>

            <div class="relative w-full max-w-md card p-6 animate-slide-up">
                <div class="flex items-center gap-4 mb-4">
                    <div class="w-12 h-12 rounded-xl bg-red-500/10 flex items-center justify-center">
                        <svg class="w-6 h-6 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
                        </svg>
                    </div>
                    <div>
                        <h3 class="text-lg font-semibold text-white">Delete Daemon</h3>
                        <p class="text-dark-400 text-sm">This action cannot be undone</p>
                    </div>
                </div>

                <p class="text-dark-300 mb-6">
                    Are you sure you want to delete <strong class="text-white">{deleteModal.daemon.name}</strong>?
                    This will affect all containers running on this daemon.
                </p>

                <div class="flex gap-3">
                    <button on:click={confirmDelete} class="btn-danger flex-1" disabled={deleting}>
                        {#if deleting}
                            <span class="spinner"></span>
                            Deleting...
                        {:else}
                            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0" />
                            </svg>
                            Delete Daemon
                        {/if}
                    </button>
                    <button on:click={closeDeleteModal} class="btn-secondary" disabled={deleting}>
                        Cancel
                    </button>
                </div>
            </div>
        </div>
    {/if}

    <!-- Edit Daemon Modal -->
    {#if showEdit && editingDaemon}
        <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
            <div
                class="absolute inset-0 bg-dark-950/80 backdrop-blur-sm"
                on:click={closeEditModal}
                on:keydown={(e) => e.key === 'Escape' && closeEditModal()}
                role="button"
                tabindex="-1"
            ></div>

            <div class="relative w-full max-w-lg card p-6 animate-slide-up">
                <div class="flex items-center justify-between mb-6">
                    <h2 class="text-xl font-semibold text-white">Edit Daemon</h2>
                    <button
                        on:click={closeEditModal}
                        class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-700/50 transition-colors duration-200"
                    >
                        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                </div>

                <form on:submit|preventDefault={updateDaemon} class="space-y-5">
                    <div class="input-group">
                        <label for="edit-name" class="input-label">Daemon Name</label>
                        <input type="text" id="edit-name" bind:value={editDaemon.name} class="input" required />
                    </div>

                    <div class="grid grid-cols-3 gap-4">
                        <div class="input-group col-span-2">
                            <label for="edit-host" class="input-label">Host</label>
                            <input type="text" id="edit-host" bind:value={editDaemon.host} class="input" required />
                            <p class="text-xs text-dark-500 mt-1">Use "daemon" for local Docker, or IP for remote</p>
                        </div>
                        <div class="input-group">
                            <label for="edit-port" class="input-label">Port</label>
                            <input type="number" id="edit-port" bind:value={editDaemon.port} class="input" required />
                        </div>
                    </div>

                    <div class="input-group">
                        <label for="edit-location" class="input-label">
                            Location
                            <span class="text-dark-500 font-normal">(optional)</span>
                        </label>
                        <input type="text" id="edit-location" bind:value={editDaemon.location} class="input" placeholder="e.g. US-East" />
                    </div>

                    <div class="flex gap-3 pt-4">
                        <button type="submit" class="btn-primary flex-1" disabled={editing}>
                            {#if editing}
                                <span class="spinner"></span>
                                Saving...
                            {:else}
                                Save Changes
                            {/if}
                        </button>
                        <button type="button" on:click={closeEditModal} class="btn-secondary">
                            Cancel
                        </button>
                    </div>
                </form>
            </div>
        </div>
    {/if}
</div>
