<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '$lib/api';
    import toast from 'svelte-french-toast';

    interface DatabaseServer {
        id: string;
        dbType: string;
        containerId: string | null;
        containerName: string;
        host: string;
        port: number;
        rootPassword?: string;
        status: string;
        databaseCount: number;
        createdAt: string;
        updatedAt: string;
    }

    let servers: DatabaseServer[] = [];
    let loading = true;
    let actionLoading: { [key: string]: boolean } = {};

    let showCreateModal = false;
    let showEditModal = false;
    let showDeleteModal = false;
    let creating = false;
    let updating = false;
    let deleting = false;

    let newServer = {
        dbType: 'postgresql',
        host: 'localhost',
        port: 5432,
        containerName: ''
    };

    let editingServer: DatabaseServer | null = null;
    let editForm = {
        host: '',
        port: 0,
        regeneratePassword: false
    };

    let deletingServer: DatabaseServer | null = null;

    const dbTypeConfig: Record<string, { name: string; defaultPort: number; color: string }> = {
        postgresql: { name: 'PostgreSQL', defaultPort: 5432, color: 'text-blue-400' },
        mysql: { name: 'MySQL', defaultPort: 3306, color: 'text-orange-400' },
        redis: { name: 'Redis', defaultPort: 6379, color: 'text-red-400' }
    };

    onMount(async () => {
        await loadServers();
    });

    async function loadServers() {
        try {
            loading = true;
            servers = await api.listDatabaseServers();
        } catch (err: any) {
            toast.error(err.message || 'Failed to load database servers');
        } finally {
            loading = false;
        }
    }

    async function createServer() {
        try {
            creating = true;
            await api.createDatabaseServer({
                dbType: newServer.dbType,
                host: newServer.host,
                port: newServer.port,
                containerName: newServer.containerName || undefined
            });
            toast.success(`${dbTypeConfig[newServer.dbType]?.name || newServer.dbType} server created`);
            showCreateModal = false;
            resetCreateForm();
            await loadServers();
        } catch (err: any) {
            toast.error(err.message || 'Failed to create server');
        } finally {
            creating = false;
        }
    }

    function resetCreateForm() {
        newServer = {
            dbType: 'postgresql',
            host: 'localhost',
            port: 5432,
            containerName: ''
        };
    }

    function onDbTypeChange() {
        const config = dbTypeConfig[newServer.dbType];
        if (config) {
            newServer.port = config.defaultPort;
        }
    }

    function openEditModal(server: DatabaseServer) {
        editingServer = server;
        editForm = {
            host: server.host,
            port: server.port,
            regeneratePassword: false
        };
        showEditModal = true;
    }

    async function updateServer() {
        if (!editingServer) return;
        try {
            updating = true;
            await api.updateDatabaseServer(editingServer.id, {
                host: editForm.host,
                port: editForm.port,
                regeneratePassword: editForm.regeneratePassword
            });
            toast.success('Server updated successfully');
            showEditModal = false;
            editingServer = null;
            await loadServers();
        } catch (err: any) {
            toast.error(err.message || 'Failed to update server');
        } finally {
            updating = false;
        }
    }

    function openDeleteModal(server: DatabaseServer) {
        deletingServer = server;
        showDeleteModal = true;
    }

    async function deleteServer() {
        if (!deletingServer) return;
        try {
            deleting = true;
            await api.deleteDatabaseServer(deletingServer.id);
            toast.success('Server deleted successfully');
            showDeleteModal = false;
            deletingServer = null;
            await loadServers();
        } catch (err: any) {
            toast.error(err.message || 'Failed to delete server');
        } finally {
            deleting = false;
        }
    }

    async function startServer(server: DatabaseServer) {
        try {
            actionLoading[server.id] = true;
            await api.startDatabaseServer(server.id);
            toast.success(`${dbTypeConfig[server.dbType]?.name || server.dbType} server started`);
            await loadServers();
        } catch (err: any) {
            toast.error(err.message || 'Failed to start server');
        } finally {
            actionLoading[server.id] = false;
        }
    }

    async function stopServer(server: DatabaseServer) {
        if (!confirm(`Are you sure you want to stop the ${dbTypeConfig[server.dbType]?.name || server.dbType} server? This will disconnect all active databases.`)) {
            return;
        }
        try {
            actionLoading[server.id] = true;
            await api.stopDatabaseServer(server.id);
            toast.success(`${dbTypeConfig[server.dbType]?.name || server.dbType} server stopped`);
            await loadServers();
        } catch (err: any) {
            toast.error(err.message || 'Failed to stop server');
        } finally {
            actionLoading[server.id] = false;
        }
    }

    function getStatusColor(status: string) {
        switch (status?.toLowerCase()) {
            case 'running': return 'badge-success';
            case 'stopped': return 'badge-danger';
            case 'starting': return 'badge-warning';
            default: return 'badge-neutral';
        }
    }

    function getStatusDot(status: string) {
        switch (status?.toLowerCase()) {
            case 'running': return 'bg-emerald-400 animate-pulse';
            case 'stopped': return 'bg-red-400';
            default: return 'bg-gray-400';
        }
    }

    function getDbTypeIcon(dbType: string) {
        switch (dbType) {
            case 'postgresql': return 'P';
            case 'mysql': return 'M';
            case 'redis': return 'R';
            default: return 'D';
        }
    }

    function getDbTypeColor(dbType: string) {
        return dbTypeConfig[dbType]?.color || 'text-gray-400';
    }

    $: existingTypes = new Set(servers.map(s => s.dbType));
    $: availableTypes = Object.keys(dbTypeConfig).filter(t => !existingTypes.has(t));
</script>

<svelte:head>
    <title>Database Servers - Admin - Raptor</title>
</svelte:head>

<div class="space-y-6">
    <div class="flex items-center justify-between">
        <div class="flex items-center gap-4">
            <a href="/admin" class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-800/50 transition-colors">
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" />
                </svg>
            </a>
            <div>
                <h1 class="text-2xl font-bold text-white">Database Servers</h1>
                <p class="text-dark-400 mt-1">Manage shared database containers for users</p>
            </div>
        </div>
        <div class="flex gap-2">
            <button on:click={loadServers} class="btn-secondary" disabled={loading}>
                <svg class="w-4 h-4 mr-2 {loading ? 'animate-spin' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
                Refresh
            </button>
            {#if availableTypes.length > 0}
                <button on:click={() => { newServer.dbType = availableTypes[0]; onDbTypeChange(); showCreateModal = true; }} class="btn-primary">
                    <svg class="w-4 h-4 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                    </svg>
                    Add Server
                </button>
            {/if}
        </div>
    </div>

    <div class="card p-4 bg-dark-800/50 border-primary-500/20">
        <div class="flex items-start gap-3">
            <svg class="w-5 h-5 text-primary-400 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <div class="text-sm text-dark-300">
                <p>Database servers are shared containers that host multiple user databases. Create and start a server to allow users to create databases of that type. Each database type can only have one server.</p>
            </div>
        </div>
    </div>

    {#if loading}
        <div class="card p-8">
            <div class="flex items-center justify-center">
                <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-500"></div>
                <span class="ml-3 text-dark-400">Loading database servers...</span>
            </div>
        </div>
    {:else if servers.length === 0}
        <div class="card p-12 text-center">
            <svg class="w-16 h-16 mx-auto text-dark-600 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                <path stroke-linecap="round" stroke-linejoin="round" d="M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375m16.5 0v3.75m-16.5-3.75v3.75m16.5 0v3.75C20.25 16.153 16.556 18 12 18s-8.25-1.847-8.25-4.125v-3.75m16.5 0c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125" />
            </svg>
            <h3 class="text-lg font-medium text-white mb-2">No database servers configured</h3>
            <p class="text-dark-400 mb-6">Create your first database server to allow users to provision databases.</p>
            <button on:click={() => { newServer.dbType = availableTypes[0] || 'postgresql'; onDbTypeChange(); showCreateModal = true; }} class="btn-primary">
                <svg class="w-4 h-4 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                </svg>
                Add Database Server
            </button>
        </div>
    {:else}
        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
            {#each servers as server}
                <div class="card p-6 space-y-4">
                    <div class="flex items-start justify-between">
                        <div class="flex items-center gap-3">
                            <div class="w-12 h-12 rounded-xl bg-dark-700 flex items-center justify-center">
                                <span class="text-2xl font-bold {getDbTypeColor(server.dbType)}">{getDbTypeIcon(server.dbType)}</span>
                            </div>
                            <div>
                                <h3 class="text-lg font-semibold text-white">{dbTypeConfig[server.dbType]?.name || server.dbType}</h3>
                                <p class="text-sm text-dark-400">{server.containerName}</p>
                            </div>
                        </div>
                        <span class="{getStatusColor(server.status)}">
                            <span class="w-2 h-2 rounded-full {getStatusDot(server.status)}"></span>
                            {server.status || 'Unknown'}
                        </span>
                    </div>

                    <div class="grid grid-cols-2 gap-4 bg-dark-800/50 rounded-lg p-4">
                        <div>
                            <span class="text-dark-500 text-xs uppercase tracking-wider">Host</span>
                            <p class="text-white font-mono text-sm mt-1">{server.host}</p>
                        </div>
                        <div>
                            <span class="text-dark-500 text-xs uppercase tracking-wider">Port</span>
                            <p class="text-white font-mono text-sm mt-1">{server.port}</p>
                        </div>
                        <div>
                            <span class="text-dark-500 text-xs uppercase tracking-wider">Databases</span>
                            <p class="text-white font-medium text-sm mt-1">{server.databaseCount || 0}</p>
                        </div>
                        <div>
                            <span class="text-dark-500 text-xs uppercase tracking-wider">Container</span>
                            <p class="text-dark-300 font-mono text-xs mt-1 truncate" title={server.containerId || 'Not created'}>
                                {server.containerId ? server.containerId.slice(0, 12) : 'Not created'}
                            </p>
                        </div>
                    </div>

                    <div class="flex items-center justify-between pt-2 border-t border-dark-700">
                        <div class="flex gap-2">
                            <button
                                on:click={() => openEditModal(server)}
                                class="p-2 text-dark-400 hover:text-white hover:bg-dark-700 rounded-lg transition-colors"
                                title="Edit"
                            >
                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M16.862 4.487l1.687-1.688a1.875 1.875 0 112.652 2.652L10.582 16.07a4.5 4.5 0 01-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 011.13-1.897l8.932-8.931zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0115.75 21H5.25A2.25 2.25 0 013 18.75V8.25A2.25 2.25 0 015.25 6H10" />
                                </svg>
                            </button>
                            <button
                                on:click={() => openDeleteModal(server)}
                                class="p-2 text-dark-400 hover:text-red-400 hover:bg-dark-700 rounded-lg transition-colors"
                                title="Delete"
                            >
                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                                </svg>
                            </button>
                        </div>
                        <div class="flex gap-2">
                            {#if server.status?.toLowerCase() === 'running'}
                                <button
                                    on:click={() => stopServer(server)}
                                    class="btn-danger btn-sm"
                                    disabled={actionLoading[server.id]}
                                >
                                    {#if actionLoading[server.id]}
                                        <svg class="animate-spin h-4 w-4 mr-1" fill="none" viewBox="0 0 24 24">
                                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                        </svg>
                                    {:else}
                                        <svg class="w-4 h-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 10a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1h-4a1 1 0 01-1-1v-4z" />
                                        </svg>
                                    {/if}
                                    Stop
                                </button>
                            {:else}
                                <button
                                    on:click={() => startServer(server)}
                                    class="btn-primary btn-sm"
                                    disabled={actionLoading[server.id]}
                                >
                                    {#if actionLoading[server.id]}
                                        <svg class="animate-spin h-4 w-4 mr-1" fill="none" viewBox="0 0 24 24">
                                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                        </svg>
                                    {:else}
                                        <svg class="w-4 h-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                                        </svg>
                                    {/if}
                                    Start
                                </button>
                            {/if}
                        </div>
                    </div>
                </div>
            {/each}
        </div>
    {/if}
</div>

{#if showCreateModal}
    <div class="fixed inset-0 z-50 flex items-center justify-center">
        <div class="absolute inset-0 bg-black/60 backdrop-blur-sm" on:click={() => showCreateModal = false}></div>
        <div class="relative bg-dark-800 rounded-xl border border-dark-700 shadow-xl w-full max-w-md mx-4 animate-fade-in">
            <div class="flex items-center justify-between p-4 border-b border-dark-700">
                <h3 class="text-lg font-semibold text-white">Add Database Server</h3>
                <button
                    on:click={() => showCreateModal = false}
                    class="text-dark-400 hover:text-white transition-colors"
                >
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
            </div>
            <form on:submit|preventDefault={createServer} class="p-4 space-y-4">
                <div>
                    <label class="text-dark-400 text-sm block mb-2">Database Type</label>
                    <div class="grid grid-cols-3 gap-3">
                        {#each availableTypes as dbType}
                            <button
                                type="button"
                                on:click={() => { newServer.dbType = dbType; onDbTypeChange(); }}
                                class="p-3 rounded-lg border-2 transition-all {newServer.dbType === dbType
                                    ? 'border-primary-500 bg-primary-500/10'
                                    : 'border-dark-600 bg-dark-700/50 hover:border-dark-500'}"
                            >
                                <span class="text-2xl font-bold block mb-1 {getDbTypeColor(dbType)}">{getDbTypeIcon(dbType)}</span>
                                <span class="text-white font-medium text-sm">{dbTypeConfig[dbType]?.name}</span>
                            </button>
                        {/each}
                    </div>
                </div>

                <div>
                    <label for="host" class="text-dark-400 text-sm block mb-2">Host</label>
                    <input
                        type="text"
                        id="host"
                        bind:value={newServer.host}
                        placeholder="localhost"
                        class="input w-full"
                        required
                    />
                    <p class="text-dark-500 text-xs mt-1">The host address for connections (usually localhost or the server IP)</p>
                </div>

                <div>
                    <label for="port" class="text-dark-400 text-sm block mb-2">Port</label>
                    <input
                        type="number"
                        id="port"
                        bind:value={newServer.port}
                        placeholder="5432"
                        class="input w-full"
                        required
                        min="1"
                        max="65535"
                    />
                </div>

                <div>
                    <label for="containerName" class="text-dark-400 text-sm block mb-2">Container Name (optional)</label>
                    <input
                        type="text"
                        id="containerName"
                        bind:value={newServer.containerName}
                        placeholder="raptor-{newServer.dbType}"
                        class="input w-full"
                    />
                    <p class="text-dark-500 text-xs mt-1">Leave empty for default name</p>
                </div>

                <div class="flex justify-end gap-3 pt-2">
                    <button
                        type="button"
                        on:click={() => showCreateModal = false}
                        class="btn-secondary"
                        disabled={creating}
                    >
                        Cancel
                    </button>
                    <button
                        type="submit"
                        class="btn-primary"
                        disabled={creating}
                    >
                        {#if creating}
                            <svg class="animate-spin -ml-1 mr-2 h-4 w-4" fill="none" viewBox="0 0 24 24">
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                            </svg>
                            Creating...
                        {:else}
                            Create Server
                        {/if}
                    </button>
                </div>
            </form>
        </div>
    </div>
{/if}

{#if showEditModal && editingServer}
    <div class="fixed inset-0 z-50 flex items-center justify-center">
        <div class="absolute inset-0 bg-black/60 backdrop-blur-sm" on:click={() => showEditModal = false}></div>
        <div class="relative bg-dark-800 rounded-xl border border-dark-700 shadow-xl w-full max-w-md mx-4 animate-fade-in">
            <div class="flex items-center justify-between p-4 border-b border-dark-700">
                <h3 class="text-lg font-semibold text-white">Edit {dbTypeConfig[editingServer.dbType]?.name} Server</h3>
                <button
                    on:click={() => showEditModal = false}
                    class="text-dark-400 hover:text-white transition-colors"
                >
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
            </div>
            <form on:submit|preventDefault={updateServer} class="p-4 space-y-4">
                <div>
                    <label for="editHost" class="text-dark-400 text-sm block mb-2">Host</label>
                    <input
                        type="text"
                        id="editHost"
                        bind:value={editForm.host}
                        placeholder="localhost"
                        class="input w-full"
                        required
                    />
                </div>

                <div>
                    <label for="editPort" class="text-dark-400 text-sm block mb-2">Port</label>
                    <input
                        type="number"
                        id="editPort"
                        bind:value={editForm.port}
                        class="input w-full"
                        required
                        min="1"
                        max="65535"
                    />
                </div>

                <div class="flex items-center gap-3 p-3 bg-dark-700/50 rounded-lg">
                    <input
                        type="checkbox"
                        id="regeneratePassword"
                        bind:checked={editForm.regeneratePassword}
                        class="w-4 h-4 rounded border-dark-600 bg-dark-700 text-primary-500 focus:ring-primary-500"
                    />
                    <label for="regeneratePassword" class="text-dark-300 text-sm">
                        Regenerate root password
                    </label>
                </div>

                {#if editForm.regeneratePassword}
                    <div class="p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg">
                        <div class="flex items-start gap-2">
                            <svg class="w-5 h-5 text-amber-400 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                            </svg>
                            <span class="text-amber-300 text-sm">This will update the root password in the running container.</span>
                        </div>
                    </div>
                {/if}

                <div class="flex justify-end gap-3 pt-2">
                    <button
                        type="button"
                        on:click={() => showEditModal = false}
                        class="btn-secondary"
                        disabled={updating}
                    >
                        Cancel
                    </button>
                    <button
                        type="submit"
                        class="btn-primary"
                        disabled={updating}
                    >
                        {#if updating}
                            <svg class="animate-spin -ml-1 mr-2 h-4 w-4" fill="none" viewBox="0 0 24 24">
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                            </svg>
                            Saving...
                        {:else}
                            Save Changes
                        {/if}
                    </button>
                </div>
            </form>
        </div>
    </div>
{/if}

{#if showDeleteModal && deletingServer}
    <div class="fixed inset-0 z-50 flex items-center justify-center">
        <div class="absolute inset-0 bg-black/60 backdrop-blur-sm" on:click={() => showDeleteModal = false}></div>
        <div class="relative bg-dark-800 rounded-xl border border-dark-700 shadow-xl w-full max-w-md mx-4 animate-fade-in">
            <div class="flex items-center justify-between p-4 border-b border-dark-700">
                <h3 class="text-lg font-semibold text-white">Delete Database Server</h3>
                <button
                    on:click={() => showDeleteModal = false}
                    class="text-dark-400 hover:text-white transition-colors"
                >
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
            </div>
            <div class="p-4 space-y-4">
                {#if deletingServer.databaseCount > 0}
                    <div class="p-3 bg-red-500/10 border border-red-500/20 rounded-lg">
                        <div class="flex items-start gap-2">
                            <svg class="w-5 h-5 text-red-400 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                            </svg>
                            <div class="text-red-300 text-sm">
                                <p class="font-medium">Cannot delete this server</p>
                                <p class="mt-1">This server has {deletingServer.databaseCount} active database(s). Delete all user databases first before removing the server.</p>
                            </div>
                        </div>
                    </div>
                {:else}
                    <p class="text-dark-300">
                        Are you sure you want to delete the <strong class="text-white">{dbTypeConfig[deletingServer.dbType]?.name}</strong> server? This will stop and remove the Docker container. This action cannot be undone.
                    </p>
                {/if}

                <div class="flex justify-end gap-3 pt-2">
                    <button
                        type="button"
                        on:click={() => showDeleteModal = false}
                        class="btn-secondary"
                    >
                        Cancel
                    </button>
                    {#if deletingServer.databaseCount === 0}
                        <button
                            on:click={deleteServer}
                            class="btn-danger"
                            disabled={deleting}
                        >
                            {#if deleting}
                                <svg class="animate-spin -ml-1 mr-2 h-4 w-4" fill="none" viewBox="0 0 24 24">
                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                </svg>
                                Deleting...
                            {:else}
                                Delete Server
                            {/if}
                        </button>
                    {/if}
                </div>
            </div>
        </div>
    </div>
{/if}
