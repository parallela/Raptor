<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '$lib/api';
    import { user } from '$lib/stores';
    import { goto } from '$app/navigation';
    import toast from 'svelte-french-toast';

    interface Flake {
        id: string;
        name: string;
        slug: string;
        author: string | null;
        description: string | null;
        dockerImage: string;
        startupCommand: string;
        configFiles: Record<string, unknown>;
        startupDetection: string | null;
        installScript: string | null;
        features: string[];
        fileDenylist: string[];
        createdAt: string;
        updatedAt: string;
    }

    interface FlakeVariable {
        id: string;
        flakeId: string;
        name: string;
        description: string | null;
        envVariable: string;
        defaultValue: string | null;
        rules: string | null;
        userViewable: boolean;
        userEditable: boolean;
        sortOrder: number;
    }

    interface FlakeWithVariables extends Flake {
        variables: FlakeVariable[];
    }

    let flakes: Flake[] = [];
    let loading = true;
    let showCreateModal = false;
    let showImportModal = false;
    let showDetailsModal = false;
    let selectedFlake: FlakeWithVariables | null = null;
    let importJson = '';
    let importing = false;

    // Create form
    let newFlake = {
        name: '',
        slug: '',
        author: '',
        description: '',
        dockerImage: 'artifacts.lstan.eu/java:21',
        startupCommand: 'java -Xms128M -Xmx{{SERVER_MEMORY}}M -jar {{SERVER_JARFILE}}',
        startupDetection: ')! For help, type ',
        installScript: '',
        variables: [] as { name: string; envVariable: string; defaultValue: string; description: string; rules: string; userViewable: boolean; userEditable: boolean }[]
    };

    $: isAdmin = $user?.roleName === 'admin';

    onMount(async () => {
        if (!$user) {
            goto('/login');
            return;
        }
        if (!isAdmin) {
            goto('/');
            toast.error('Admin access required');
            return;
        }
        await loadFlakes();
    });

    async function loadFlakes() {
        loading = true;
        try {
            flakes = await api.listFlakes();
        } catch (e: any) {
            toast.error(e.message || 'Failed to load flakes');
        } finally {
            loading = false;
        }
    }

    async function viewFlake(id: string) {
        try {
            selectedFlake = await api.getFlake(id);
            showDetailsModal = true;
        } catch (e: any) {
            toast.error(e.message || 'Failed to load flake');
        }
    }

    async function createFlake() {
        try {
            await api.createFlake({
                name: newFlake.name,
                slug: newFlake.slug || newFlake.name.toLowerCase().replace(/\s+/g, '-'),
                author: newFlake.author || null,
                description: newFlake.description || null,
                dockerImage: newFlake.dockerImage,
                startupCommand: newFlake.startupCommand,
                startupDetection: newFlake.startupDetection || null,
                installScript: newFlake.installScript || null,
                configFiles: {},
                variables: newFlake.variables
            });
            toast.success('Flake created');
            showCreateModal = false;
            resetForm();
            await loadFlakes();
        } catch (e: any) {
            toast.error(e.message || 'Failed to create flake');
        }
    }

    async function importFlake() {
        importing = true;
        try {
            const flakeJson = JSON.parse(importJson);
            await api.importFlake(flakeJson);
            toast.success('Flake imported successfully');
            showImportModal = false;
            importJson = '';
            await loadFlakes();
        } catch (e: any) {
            toast.error(e.message || 'Failed to import flake');
        } finally {
            importing = false;
        }
    }

    async function deleteFlake(id: string) {
        if (!confirm('Are you sure you want to delete this flake?')) return;
        try {
            await api.deleteFlake(id);
            toast.success('Flake deleted');
            await loadFlakes();
        } catch (e: any) {
            toast.error(e.message || 'Failed to delete flake');
        }
    }

    async function exportFlake(id: string) {
        try {
            const flake = await api.exportFlake(id);
            const blob = new Blob([JSON.stringify(flake, null, 2)], { type: 'application/json' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `flake-${id}.json`;
            a.click();
            URL.revokeObjectURL(url);
            toast.success('Flake exported');
        } catch (e: any) {
            toast.error(e.message || 'Failed to export flake');
        }
    }

    function resetForm() {
        newFlake = {
            name: '',
            slug: '',
            author: '',
            description: '',
            dockerImage: 'artifacts.lstan.eu/java:21',
            startupCommand: 'java -Xms128M -Xmx{{SERVER_MEMORY}}M -jar {{SERVER_JARFILE}}',
            startupDetection: ')! For help, type ',
            installScript: '',
            variables: []
        };
    }

    function addVariable() {
        newFlake.variables = [...newFlake.variables, {
            name: '',
            envVariable: '',
            defaultValue: '',
            description: '',
            rules: 'nullable|string',
            userViewable: true,
            userEditable: true
        }];
    }

    function removeVariable(index: number) {
        newFlake.variables = newFlake.variables.filter((_, i) => i !== index);
    }
</script>

<svelte:head>
    <title>Flakes - Raptor Panel</title>
</svelte:head>

<div class="p-6 max-w-7xl mx-auto">
    <div class="flex justify-between items-center mb-6">
        <div>
            <h1 class="text-2xl font-bold text-white">Flakes</h1>
            <p class="text-dark-400 text-sm">Server templates (like Pterodactyl Eggs)</p>
        </div>
        <div class="flex gap-2">
            <button on:click={() => showImportModal = true} class="btn-secondary">
                Import Flake
            </button>
            <button on:click={() => showCreateModal = true} class="btn-primary">
                Create Flake
            </button>
        </div>
    </div>

    {#if loading}
        <div class="flex justify-center py-12">
            <span class="spinner w-8 h-8"></span>
        </div>
    {:else if flakes.length === 0}
        <div class="bg-dark-800 rounded-lg p-12 text-center">
            <p class="text-dark-400">No flakes yet. Create one or import a flake.</p>
        </div>
    {:else}
        <div class="grid gap-4">
            {#each flakes as flake}
                <div class="bg-dark-800 rounded-lg p-4 hover:bg-dark-750 transition-colors">
                    <div class="flex justify-between items-start">
                        <div class="flex-1">
                            <div class="flex items-center gap-2">
                                <h3 class="text-lg font-semibold text-white">{flake.name}</h3>
                                <span class="px-2 py-0.5 text-xs bg-dark-700 text-dark-300 rounded">{flake.slug}</span>
                            </div>
                            {#if flake.description}
                                <p class="text-dark-400 text-sm mt-1">{flake.description}</p>
                            {/if}
                            <div class="flex items-center gap-4 mt-2 text-xs text-dark-500">
                                <span>Image: <code class="text-primary-400">{flake.dockerImage}</code></span>
                                {#if flake.author}
                                    <span>by {flake.author}</span>
                                {/if}
                            </div>
                        </div>
                        <div class="flex gap-2">
                            <button on:click={() => viewFlake(flake.id)} class="btn-sm btn-secondary">
                                View
                            </button>
                            <button on:click={() => exportFlake(flake.id)} class="btn-sm btn-secondary">
                                Export
                            </button>
                            <button on:click={() => deleteFlake(flake.id)} class="btn-sm btn-danger">
                                Delete
                            </button>
                        </div>
                    </div>
                </div>
            {/each}
        </div>
    {/if}
</div>

<!-- Create Modal -->
{#if showCreateModal}
    <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
        <div class="bg-dark-800 rounded-lg w-full max-w-2xl max-h-[90vh] overflow-y-auto">
            <div class="p-4 border-b border-dark-700">
                <h2 class="text-lg font-semibold text-white">Create Flake</h2>
            </div>
            <form on:submit|preventDefault={createFlake} class="p-4 space-y-4">
                <div class="grid grid-cols-2 gap-4">
                    <div>
                        <label class="block text-sm text-dark-400 mb-1">Name</label>
                        <input type="text" bind:value={newFlake.name} required class="input w-full" placeholder="Paper" />
                    </div>
                    <div>
                        <label class="block text-sm text-dark-400 mb-1">Slug</label>
                        <input type="text" bind:value={newFlake.slug} class="input w-full" placeholder="paper" />
                    </div>
                </div>
                <div>
                    <label class="block text-sm text-dark-400 mb-1">Description</label>
                    <textarea bind:value={newFlake.description} class="input w-full" rows="2"></textarea>
                </div>
                <div>
                    <label class="block text-sm text-dark-400 mb-1">Docker Image</label>
                    <input type="text" bind:value={newFlake.dockerImage} required class="input w-full" />
                </div>
                <div>
                    <label class="block text-sm text-dark-400 mb-1">Startup Command</label>
                    <input type="text" bind:value={newFlake.startupCommand} required class="input w-full font-mono text-sm" />
                    <p class="text-xs text-dark-500 mt-1">Use {"{{VARIABLE}}"} for variable substitution</p>
                </div>
                <div>
                    <label class="block text-sm text-dark-400 mb-1">Startup Detection String</label>
                    <input type="text" bind:value={newFlake.startupDetection} class="input w-full" placeholder=")! For help, type " />
                </div>
                <div>
                    <label class="block text-sm text-dark-400 mb-1">Install Script (optional)</label>
                    <textarea bind:value={newFlake.installScript} class="input w-full font-mono text-sm" rows="4"></textarea>
                </div>

                <div>
                    <div class="flex justify-between items-center mb-2">
                        <label class="block text-sm text-dark-400">Variables</label>
                        <button type="button" on:click={addVariable} class="btn-sm btn-secondary">Add Variable</button>
                    </div>
                    {#each newFlake.variables as variable, i}
                        <div class="bg-dark-700 rounded p-3 mb-2">
                            <div class="grid grid-cols-2 gap-2 mb-2">
                                <input type="text" bind:value={variable.name} placeholder="Name" class="input" />
                                <input type="text" bind:value={variable.envVariable} placeholder="ENV_VAR" class="input font-mono" />
                            </div>
                            <div class="grid grid-cols-2 gap-2 mb-2">
                                <input type="text" bind:value={variable.defaultValue} placeholder="Default value" class="input" />
                                <input type="text" bind:value={variable.rules} placeholder="Rules" class="input" />
                            </div>
                            <div class="flex justify-between items-center">
                                <div class="flex gap-4">
                                    <label class="flex items-center gap-1 text-sm text-dark-400">
                                        <input type="checkbox" bind:checked={variable.userViewable} />
                                        Viewable
                                    </label>
                                    <label class="flex items-center gap-1 text-sm text-dark-400">
                                        <input type="checkbox" bind:checked={variable.userEditable} />
                                        Editable
                                    </label>
                                </div>
                                <button type="button" on:click={() => removeVariable(i)} class="text-red-500 text-sm">Remove</button>
                            </div>
                        </div>
                    {/each}
                </div>

                <div class="flex justify-end gap-2 pt-4 border-t border-dark-700">
                    <button type="button" on:click={() => { showCreateModal = false; resetForm(); }} class="btn-secondary">Cancel</button>
                    <button type="submit" class="btn-primary">Create</button>
                </div>
            </form>
        </div>
    </div>
{/if}

<!-- Import Modal -->
{#if showImportModal}
    <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
        <div class="bg-dark-800 rounded-lg w-full max-w-2xl">
            <div class="p-4 border-b border-dark-700">
                <h2 class="text-lg font-semibold text-white">Import Flake</h2>
            </div>
            <form on:submit|preventDefault={importFlake} class="p-4">
                <p class="text-dark-400 text-sm mb-4">
                    Paste the JSON content of a Flake (or Pterodactyl egg) to import it.
                </p>
                <textarea
                    bind:value={importJson}
                    required
                    class="input w-full font-mono text-sm"
                    rows="12"
                    placeholder="Paste Flake JSON here..."
                ></textarea>
                <div class="flex justify-end gap-2 mt-4">
                    <button type="button" on:click={() => { showImportModal = false; importJson = ''; }} class="btn-secondary">Cancel</button>
                    <button type="submit" disabled={importing} class="btn-primary">
                        {#if importing}
                            <span class="spinner w-4 h-4"></span>
                        {:else}
                            Import
                        {/if}
                    </button>
                </div>
            </form>
        </div>
    </div>
{/if}

<!-- Details Modal -->
{#if showDetailsModal && selectedFlake}
    <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
        <div class="bg-dark-800 rounded-lg w-full max-w-2xl max-h-[90vh] overflow-y-auto">
            <div class="p-4 border-b border-dark-700 flex justify-between items-center">
                <h2 class="text-lg font-semibold text-white">{selectedFlake.name}</h2>
                <button on:click={() => { showDetailsModal = false; selectedFlake = null; }} class="text-dark-400 hover:text-white">
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
            </div>
            <div class="p-4 space-y-4">
                {#if selectedFlake.description}
                    <p class="text-dark-400">{selectedFlake.description}</p>
                {/if}

                <div>
                    <h3 class="text-sm font-medium text-dark-300 mb-1">Docker Image</h3>
                    <code class="text-primary-400 text-sm">{selectedFlake.dockerImage}</code>
                </div>

                <div>
                    <h3 class="text-sm font-medium text-dark-300 mb-1">Startup Command</h3>
                    <code class="text-sm text-white bg-dark-700 px-2 py-1 rounded block overflow-x-auto">{selectedFlake.startupCommand}</code>
                </div>

                {#if selectedFlake.variables.length > 0}
                    <div>
                        <h3 class="text-sm font-medium text-dark-300 mb-2">Variables</h3>
                        <div class="space-y-2">
                            {#each selectedFlake.variables as variable}
                                <div class="bg-dark-700 rounded p-3">
                                    <div class="flex justify-between items-start">
                                        <div>
                                            <span class="text-white font-medium">{variable.name}</span>
                                            <code class="text-xs text-primary-400 ml-2">{variable.envVariable}</code>
                                        </div>
                                        <div class="flex gap-2 text-xs">
                                            {#if variable.userViewable}
                                                <span class="text-green-500">Viewable</span>
                                            {/if}
                                            {#if variable.userEditable}
                                                <span class="text-blue-500">Editable</span>
                                            {/if}
                                        </div>
                                    </div>
                                    {#if variable.description}
                                        <p class="text-dark-400 text-sm mt-1">{variable.description}</p>
                                    {/if}
                                    <div class="flex gap-4 mt-2 text-xs text-dark-500">
                                        <span>Default: <code>{variable.defaultValue || '(none)'}</code></span>
                                        {#if variable.rules}
                                            <span>Rules: {variable.rules}</span>
                                        {/if}
                                    </div>
                                </div>
                            {/each}
                        </div>
                    </div>
                {/if}

                {#if selectedFlake.installScript}
                    <div>
                        <h3 class="text-sm font-medium text-dark-300 mb-1">Install Script</h3>
                        <pre class="text-xs text-white bg-dark-700 p-3 rounded overflow-x-auto max-h-48">{selectedFlake.installScript}</pre>
                    </div>
                {/if}
            </div>
        </div>
    </div>
{/if}

<style>
    .btn-sm {
        @apply px-2 py-1 text-sm rounded;
    }
    .btn-secondary {
        @apply bg-dark-700 text-white hover:bg-dark-600;
    }
    .btn-primary {
        @apply bg-primary-600 text-white hover:bg-primary-500;
    }
    .btn-danger {
        @apply bg-red-600 text-white hover:bg-red-500;
    }
    .input {
        @apply bg-dark-700 border border-dark-600 rounded px-3 py-2 text-white focus:outline-none focus:border-primary-500;
    }
</style>
