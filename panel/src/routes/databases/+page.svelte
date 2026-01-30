<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '$lib/api';
    import toast from 'svelte-french-toast';

    interface Database {
        id: string;
        dbType: string;
        dbName: string;
        dbUser: string;
        dbPassword: string;
        host: string;
        port: number;
        status: string;
        connectionString: string;
        createdAt: string;
    }

    interface AvailableDbType {
        dbType: string;
        name: string;
        available: boolean;
    }

    let databases: Database[] = [];
    let availableTypes: AvailableDbType[] = [];
    let loading = true;
    let showCreateModal = false;
    let creating = false;
    let selectedDbType = '';
    let customDbName = '';
    let showPassword: { [key: string]: boolean } = {};
    let copiedField: string | null = null;

    const dbTypeConfig: Record<string, { name: string; color: string; icon: string }> = {
        postgresql: { name: 'PostgreSQL', color: 'text-blue-400', icon: 'P' },
        mysql: { name: 'MySQL', color: 'text-orange-400', icon: 'M' },
        redis: { name: 'Redis', color: 'text-red-400', icon: 'R' }
    };

    onMount(async () => {
        await loadData();
    });

    async function loadData() {
        try {
            loading = true;
            [databases, availableTypes] = await Promise.all([
                api.listDatabases(),
                api.getAvailableDatabaseTypes()
            ]);
            const firstAvailable = availableTypes.find(t => t.available);
            if (firstAvailable) {
                selectedDbType = firstAvailable.dbType;
            }
        } catch (err: any) {
            toast.error(err.message || 'Failed to load databases');
        } finally {
            loading = false;
        }
    }

    async function createDatabase() {
        try {
            creating = true;
            const payload: { dbType: string; dbName?: string } = { dbType: selectedDbType };
            if (customDbName.trim() && selectedDbType !== 'redis') {
                payload.dbName = customDbName.trim();
            }
            await api.createDatabase(payload);
            const typeName = dbTypeConfig[selectedDbType]?.name || selectedDbType;
            toast.success(`${typeName} database created successfully!`);
            showCreateModal = false;
            customDbName = '';
            await loadData();
        } catch (err: any) {
            toast.error(err.message || 'Failed to create database');
        } finally {
            creating = false;
        }
    }

    async function deleteDatabase(db: Database) {
        if (!confirm(`Are you sure you want to delete the database "${db.dbName}"? This action cannot be undone.`)) {
            return;
        }
        try {
            await api.deleteDatabase(db.id);
            toast.success('Database deleted successfully');
            await loadData();
        } catch (err: any) {
            toast.error(err.message || 'Failed to delete database');
        }
    }

    async function resetPassword(db: Database) {
        if (!confirm(`Are you sure you want to reset the password for "${db.dbName}"? You will need to update your application's connection settings.`)) {
            return;
        }
        try {
            await api.resetDatabasePassword(db.id);
            toast.success('Password reset successfully');
            await loadData();
        } catch (err: any) {
            toast.error(err.message || 'Failed to reset password');
        }
    }

    function copyToClipboard(text: string, fieldId: string) {
        navigator.clipboard.writeText(text);
        copiedField = fieldId;
        toast.success('Copied to clipboard');
        setTimeout(() => {
            copiedField = null;
        }, 2000);
    }

    function togglePassword(id: string) {
        showPassword[id] = !showPassword[id];
    }

    function getDbIcon(type: string) {
        return dbTypeConfig[type]?.icon || 'D';
    }

    function getStatusColor(status: string) {
        switch (status) {
            case 'active': return 'bg-green-500';
            case 'creating': return 'bg-yellow-500';
            case 'error': return 'bg-red-500';
            default: return 'bg-gray-500';
        }
    }

    $: canCreateDatabase = availableTypes.some(t => t.available);
</script>

<svelte:head>
    <title>Databases - Raptor</title>
</svelte:head>

<div class="space-y-6">
    <div class="flex items-center justify-between">
        <div>
            <h1 class="text-2xl font-bold text-white">Databases</h1>
            <p class="text-dark-400 mt-1">Manage your PostgreSQL, MySQL, and Redis databases</p>
        </div>
        {#if canCreateDatabase}
            <button
                on:click={() => showCreateModal = true}
                class="btn-primary flex items-center gap-2"
            >
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                </svg>
                Create Database
            </button>
        {:else if !loading}
            <div class="text-dark-400 text-sm">No database servers available</div>
        {/if}
    </div>

    <!-- Loading state -->
    {#if loading}
        <div class="card p-8">
            <div class="flex items-center justify-center">
                <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-500"></div>
                <span class="ml-3 text-dark-400">Loading databases...</span>
            </div>
        </div>
    {:else if databases.length === 0}
        <!-- Empty state -->
        <div class="card p-12 text-center">
            <svg class="w-16 h-16 mx-auto text-dark-600 mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                <path stroke-linecap="round" stroke-linejoin="round" d="M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375m16.5 0v3.75m-16.5-3.75v3.75m16.5 0v3.75C20.25 16.153 16.556 18 12 18s-8.25-1.847-8.25-4.125v-3.75m16.5 0c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125" />
            </svg>
            <h3 class="text-lg font-medium text-white mb-2">No databases yet</h3>
            {#if canCreateDatabase}
                <p class="text-dark-400 mb-6">Create your first database to get started.</p>
                <button on:click={() => showCreateModal = true} class="btn-primary">
                    Create Database
                </button>
            {:else}
                <p class="text-dark-400">No database servers are available. Contact your administrator to enable database services.</p>
            {/if}
        </div>
    {:else}
        <!-- Database cards -->
        <div class="grid gap-6 md:grid-cols-2">
            {#each databases as db}
                <div class="card p-6 space-y-4">
                    <!-- Header -->
                    <div class="flex items-start justify-between">
                        <div class="flex items-center gap-3">
                            <img
                                src="/svg-icons/{db.dbType}.svg"
                                alt={db.dbType}
                                class="w-10 h-10"
                            />
                            <div>
                                <h3 class="text-lg font-semibold text-white">{db.dbName}</h3>
                                <p class="text-sm text-dark-400 capitalize">{db.dbType}</p>
                            </div>
                        </div>
                        <div class="flex items-center gap-2">
                            <span class={`w-2.5 h-2.5 rounded-full ${getStatusColor(db.status)}`}></span>
                            <span class="text-sm text-dark-400 capitalize">{db.status}</span>
                        </div>
                    </div>

                    <!-- Connection details -->
                    <div class="space-y-3 bg-dark-800/50 rounded-lg p-4">
                        <div class="flex items-center justify-between">
                            <span class="text-dark-400 text-sm">Host</span>
                            <div class="flex items-center gap-2">
                                <code class="text-sm text-white bg-dark-700 px-2 py-1 rounded">{db.host}:{db.port}</code>
                                <button
                                    on:click={() => copyToClipboard(`${db.host}:${db.port}`, `host-${db.id}`)}
                                    class="text-dark-400 hover:text-white transition-colors"
                                    title="Copy"
                                >
                                    {#if copiedField === `host-${db.id}`}
                                        <svg class="w-4 h-4 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                                        </svg>
                                    {:else}
                                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                                        </svg>
                                    {/if}
                                </button>
                            </div>
                        </div>

                        <div class="flex items-center justify-between">
                            <span class="text-dark-400 text-sm">Database</span>
                            <div class="flex items-center gap-2">
                                <code class="text-sm text-white bg-dark-700 px-2 py-1 rounded">{db.dbName}</code>
                                <button
                                    on:click={() => copyToClipboard(db.dbName, `dbname-${db.id}`)}
                                    class="text-dark-400 hover:text-white transition-colors"
                                    title="Copy"
                                >
                                    {#if copiedField === `dbname-${db.id}`}
                                        <svg class="w-4 h-4 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                                        </svg>
                                    {:else}
                                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                                        </svg>
                                    {/if}
                                </button>
                            </div>
                        </div>

                        <div class="flex items-center justify-between">
                            <span class="text-dark-400 text-sm">Username</span>
                            <div class="flex items-center gap-2">
                                <code class="text-sm text-white bg-dark-700 px-2 py-1 rounded">{db.dbUser}</code>
                                <button
                                    on:click={() => copyToClipboard(db.dbUser, `user-${db.id}`)}
                                    class="text-dark-400 hover:text-white transition-colors"
                                    title="Copy"
                                >
                                    {#if copiedField === `user-${db.id}`}
                                        <svg class="w-4 h-4 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                                        </svg>
                                    {:else}
                                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                                        </svg>
                                    {/if}
                                </button>
                            </div>
                        </div>

                        <div class="flex items-center justify-between">
                            <span class="text-dark-400 text-sm">Password</span>
                            <div class="flex items-center gap-2">
                                <code class="text-sm text-white bg-dark-700 px-2 py-1 rounded font-mono">
                                    {showPassword[db.id] ? db.dbPassword : '••••••••••••'}
                                </code>
                                <button
                                    on:click={() => togglePassword(db.id)}
                                    class="text-dark-400 hover:text-white transition-colors"
                                    title={showPassword[db.id] ? 'Hide' : 'Show'}
                                >
                                    {#if showPassword[db.id]}
                                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21" />
                                        </svg>
                                    {:else}
                                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                                        </svg>
                                    {/if}
                                </button>
                                <button
                                    on:click={() => copyToClipboard(db.dbPassword, `pass-${db.id}`)}
                                    class="text-dark-400 hover:text-white transition-colors"
                                    title="Copy"
                                >
                                    {#if copiedField === `pass-${db.id}`}
                                        <svg class="w-4 h-4 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                                        </svg>
                                    {:else}
                                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                                        </svg>
                                    {/if}
                                </button>
                            </div>
                        </div>
                    </div>

                    <!-- Connection string -->
                    <div class="space-y-2">
                        <div class="flex items-center justify-between">
                            <span class="text-dark-400 text-sm">Connection String</span>
                            <button
                                on:click={() => copyToClipboard(db.connectionString, `conn-${db.id}`)}
                                class="text-sm text-primary-400 hover:text-primary-300 flex items-center gap-1"
                            >
                                {#if copiedField === `conn-${db.id}`}
                                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                                    </svg>
                                    Copied!
                                {:else}
                                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                                    </svg>
                                    Copy
                                {/if}
                            </button>
                        </div>
                        <code class="block text-xs text-dark-300 bg-dark-800 p-3 rounded-lg break-all font-mono">
                            {db.connectionString.replace(db.dbPassword, '••••••••')}
                        </code>
                    </div>

                    <!-- Redis Key Prefix Notice -->
                    {#if db.dbType === 'redis'}
                        <div class="bg-red-500/10 border border-red-500/20 rounded-lg p-3">
                            <div class="flex items-start gap-2">
                                <svg class="w-5 h-5 text-red-400 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                                </svg>
                                <div class="text-sm">
                                    <p class="text-red-300 font-medium">Key Prefix Required</p>
                                    <p class="text-red-400/80 mt-1">All keys must be prefixed with <code class="bg-dark-800 px-1.5 py-0.5 rounded text-red-300">{db.dbName}:</code></p>
                                    <p class="text-dark-400 text-xs mt-2">Example: <code class="bg-dark-800 px-1.5 py-0.5 rounded">{db.dbName}:mykey</code></p>
                                </div>
                            </div>
                        </div>
                    {/if}

                    <!-- Actions -->
                    <div class="flex items-center justify-between pt-2 border-t border-dark-700">
                        <div class="flex gap-2">
                            <button
                                on:click={() => resetPassword(db)}
                                class="btn-secondary text-sm"
                            >
                                <svg class="w-4 h-4 mr-1.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                                </svg>
                                Reset Password
                            </button>
                        </div>
                        <button
                            on:click={() => deleteDatabase(db)}
                            class="text-red-400 hover:text-red-300 text-sm flex items-center"
                        >
                            <svg class="w-4 h-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                            </svg>
                            Delete
                        </button>
                    </div>
                </div>
            {/each}
        </div>
    {/if}
</div>

<!-- Create Database Modal -->
{#if showCreateModal}
    <div class="fixed inset-0 z-50 flex items-center justify-center">
        <div class="absolute inset-0 bg-black/60 backdrop-blur-sm" on:click={() => showCreateModal = false}></div>
        <div class="relative bg-dark-800 rounded-xl border border-dark-700 shadow-xl w-full max-w-md mx-4 animate-fade-in">
            <div class="flex items-center justify-between p-4 border-b border-dark-700">
                <h3 class="text-lg font-semibold text-white">Create Database</h3>
                <button
                    on:click={() => showCreateModal = false}
                    class="text-dark-400 hover:text-white transition-colors"
                >
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
            </div>
            <form on:submit|preventDefault={createDatabase} class="p-4 space-y-4">
                <div>
                    <label class="text-dark-400 text-sm block mb-2">Database Type</label>
                    <div class="grid grid-cols-{availableTypes.filter(t => t.available).length > 2 ? '3' : '2'} gap-3">
                        {#each availableTypes.filter(t => t.available) as dbType}
                            <button
                                type="button"
                                on:click={() => selectedDbType = dbType.dbType}
                                class="p-4 rounded-lg border-2 transition-all {selectedDbType === dbType.dbType
                                    ? 'border-primary-500 bg-primary-500/10'
                                    : 'border-dark-600 bg-dark-700/50 hover:border-dark-500'}"
                            >
                                <img
                                    src="/svg-icons/{dbType.dbType}.svg"
                                    alt={dbType.name}
                                    class="w-10 h-10 mx-auto mb-2"
                                />
                                <span class="text-white font-medium">{dbType.name}</span>
                                <span class="text-dark-400 text-xs block mt-1">{dbType.dbType === 'postgresql' ? 'v16' : dbType.dbType === 'mysql' ? 'v8.0' : 'v7'}</span>
                            </button>
                        {/each}
                    </div>
                    {#if availableTypes.filter(t => t.available).length === 0}
                        <p class="text-dark-500 text-sm mt-2">No database types available. Contact an administrator.</p>
                    {/if}
                </div>

                {#if selectedDbType !== 'redis'}
                    <div>
                        <label for="dbName" class="text-dark-400 text-sm block mb-2">Database Name (optional)</label>
                        <input
                            type="text"
                            id="dbName"
                            bind:value={customDbName}
                            placeholder="Leave empty for auto-generated name"
                            class="input w-full"
                        />
                        <p class="text-dark-500 text-xs mt-1">Only letters, numbers, and underscores allowed</p>
                    </div>
                {/if}

                <div class="bg-dark-700/50 rounded-lg p-3 text-sm">
                    <div class="flex items-start gap-2">
                        <svg class="w-5 h-5 text-primary-400 flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                        <div class="text-dark-300">
                            {#if selectedDbType === 'redis'}
                                <p>Your Redis instance will be created with:</p>
                                <ul class="list-disc list-inside mt-1 text-dark-400">
                                    <li>Dedicated database slot</li>
                                    <li>Auto-generated secure credentials</li>
                                    <li>ACL-based access control</li>
                                </ul>
                            {:else}
                                <p>Your database will be created with:</p>
                                <ul class="list-disc list-inside mt-1 text-dark-400">
                                    <li>Dedicated user and permissions</li>
                                    <li>Auto-generated secure password</li>
                                    <li>Full database access</li>
                                </ul>
                            {/if}
                        </div>
                    </div>
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
                            Create Database
                        {/if}
                    </button>
                </div>
            </form>
        </div>
    </div>
{/if}
