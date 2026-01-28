<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '$lib/api';
    import { user } from '$lib/stores';
    import { Select } from '$lib/components';
    import UserSearch from '$lib/components/UserSearch.svelte';
    import { goto } from '$app/navigation';
    import toast from 'svelte-french-toast';
    import type { Container, Daemon, User, Flake, FlakeVariable } from '$lib/types';

    interface FlakeWithVariables extends Flake {
        variables: FlakeVariable[];
    }

    let containers: Container[] = [];
    let daemons: Daemon[] = [];
    let flakes: Flake[] = [];
    let loading = true;
    let showCreate = false;
    let creating = false;
    let selectedFlake: FlakeWithVariables | null = null;
    let selectedUser: User | null = null;
    let flakeVariables: Record<string, string> = {};

    let newContainer = {
        daemonId: '',
        name: '',
        flakeId: '',
        image: '',
        startupScript: '',
        allocationId: '',
        memoryLimit: 1024,
        cpuLimit: 1.0,
        diskLimit: 5120,
        userId: ''
    };

    // Check if current user is admin/manager
    $: isAdmin = $user?.roleName === 'admin' || $user?.roleName === 'manager';

    onMount(async () => {
        if (!$user) {
            goto('/login');
            return;
        }
        await loadData();
    });

    async function loadData() {
        loading = true;
        try {
            [containers, daemons, flakes] = await Promise.all([
                api.listContainers(),
                api.listDaemons(),
                api.listFlakes()
            ]);
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    }

    async function selectFlake(flakeId: string) {
        if (!flakeId) {
            selectedFlake = null;
            flakeVariables = {};
            newContainer.image = '';
            newContainer.startupScript = '';
            return;
        }
        try {
            selectedFlake = await api.getFlake(flakeId);
            // Initialize variables with defaults
            flakeVariables = {};
            if (selectedFlake) {
                for (const v of selectedFlake.variables) {
                    flakeVariables[v.envVariable] = v.defaultValue || '';
                }
                // Set SERVER_MEMORY from memory limit
                flakeVariables['SERVER_MEMORY'] = String(newContainer.memoryLimit);
                // Pre-fill startup command from flake
                newContainer.startupScript = selectedFlake.startupCommand;
            }
        } catch (e: any) {
            toast.error('Failed to load flake details');
        }
    }

    async function createContainer() {
        creating = true;
        try {
            const payload: Record<string, unknown> = {
                daemonId: newContainer.daemonId,
                name: newContainer.name,
                allocationId: newContainer.allocationId || undefined,
                memoryLimit: newContainer.memoryLimit,
                cpuLimit: newContainer.cpuLimit,
                diskLimit: newContainer.diskLimit,
                userId: newContainer.userId || undefined,
                startupScript: newContainer.startupScript || undefined
            };

            // If flake selected, use flake_id and variables
            if (newContainer.flakeId && selectedFlake) {
                payload.flakeId = newContainer.flakeId;
                payload.variables = flakeVariables;
            } else {
                // Manual mode - require image
                payload.image = newContainer.image;
            }

            await api.createContainer(payload as any);
            showCreate = false;
            newContainer = { daemonId: '', name: '', flakeId: '', image: '', startupScript: '', allocationId: '', memoryLimit: 1024, cpuLimit: 1.0, diskLimit: 5120, userId: '' };
            selectedFlake = null;
            flakeVariables = {};
            selectedUser = null;
            await loadData();
            toast.success('Server created successfully');
        } catch (e: any) {
            toast.error(e.message || 'Failed to create server');
        } finally {
            creating = false;
        }
    }

    // Delete confirmation state
    let deleteTarget: { id: string; name: string } | null = null;
    let deleteStep = 0;
    let deleteConfirmName = '';
    let deleting = false;

    function startDelete(id: string, name: string) {
        deleteTarget = { id, name };
        deleteStep = 1;
        deleteConfirmName = '';
    }

    function cancelDelete() {
        deleteTarget = null;
        deleteStep = 0;
        deleteConfirmName = '';
    }

    async function confirmDelete() {
        if (!deleteTarget) return;

        if (deleteStep < 3) {
            deleteStep++;
            return;
        }

        // Final step - verify name matches
        if (deleteConfirmName !== deleteTarget.name) {
            toast.error('Server name does not match');
            return;
        }

        deleting = true;
        try {
            await api.deleteContainer(deleteTarget.id);
            await loadData();
            toast.success('Server deleted permanently');
            cancelDelete();
        } catch (e: any) {
            toast.error(e.message || 'Failed to delete server');
        } finally {
            deleting = false;
        }
    }

    function getStatusBadge(status: string) {
        switch (status.toLowerCase()) {
            case 'running': return 'badge-success';
            case 'stopped': return 'badge-danger';
            default: return 'badge-neutral';
        }
    }

    // Allocations for create modal (fetched when daemon is selected)
    let availableAllocations: { id: string; ip: string; port: number }[] = [];
    let loadingAllocations = false;

    async function loadAvailableAllocations(daemonId: string) {
        if (!daemonId) {
            availableAllocations = [];
            return;
        }
        loadingAllocations = true;
        try {
            const allAllocations = await api.listAllocations();
            // Filter to only show allocations for this daemon that aren't assigned
            availableAllocations = allAllocations.filter(a => a.daemonId === daemonId);
        } catch (e) {
            console.error('Failed to load allocations:', e);
            availableAllocations = [];
        } finally {
            loadingAllocations = false;
        }
    }

    // Watch for daemon selection changes
    $: if (newContainer.daemonId) {
        loadAvailableAllocations(newContainer.daemonId);
    }
</script>

<div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
        <div>
            <h1 class="section-title">Servers</h1>
            <p class="section-subtitle">Manage your game servers and applications</p>
        </div>
        <button on:click={() => showCreate = true} class="btn-primary">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
            Create Server
        </button>
    </div>

    <!-- Create Modal -->
    {#if showCreate}
        <div class="fixed inset-0 z-50 overflow-y-auto">
            <!-- Backdrop -->
            <div
                class="fixed inset-0 bg-dark-950/80 backdrop-blur-sm"
                on:click={() => showCreate = false}
                on:keydown={(e) => e.key === 'Escape' && (showCreate = false)}
                role="button"
                tabindex="-1"
            ></div>

            <!-- Modal Container -->
            <div class="flex min-h-full items-start justify-center p-4 pt-20 pb-8">
                <!-- Modal -->
                <div class="relative w-full max-w-2xl card p-6 animate-slide-up">
                <div class="flex items-center justify-between mb-6">
                    <h2 class="text-xl font-semibold text-white">Create New Server</h2>
                    <button
                        on:click={() => showCreate = false}
                        class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-700/50 transition-colors duration-200"
                    >
                        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                </div>

                <form on:submit|preventDefault={createContainer} class="space-y-5">
                    <!-- Flake Selection -->
                    <div class="input-group">
                        <label class="input-label">Server Type (Flake)</label>
                        {#if flakes.length > 0}
                            <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
                                {#each flakes as flake}
                                    <button
                                        type="button"
                                        on:click={() => { newContainer.flakeId = flake.id; selectFlake(flake.id); }}
                                        class="p-3 rounded-lg border text-left transition-all duration-200 {newContainer.flakeId === flake.id ? 'border-primary-500 bg-primary-500/10' : 'border-dark-700 bg-dark-800/50 hover:border-dark-600'}"
                                    >
                                        <p class="text-sm font-medium text-white">{flake.name}</p>
                                        <p class="text-xs text-dark-400 mt-0.5 truncate">{flake.description || flake.dockerImage}</p>
                                    </button>
                                {/each}
                                <button
                                    type="button"
                                    on:click={() => { newContainer.flakeId = ''; selectedFlake = null; flakeVariables = {}; }}
                                    class="p-3 rounded-lg border text-left transition-all duration-200 {!newContainer.flakeId ? 'border-primary-500 bg-primary-500/10' : 'border-dark-700 bg-dark-800/50 hover:border-dark-600'}"
                                >
                                    <p class="text-sm font-medium text-white">Custom Image</p>
                                    <p class="text-xs text-dark-400 mt-0.5">Use your own Docker image</p>
                                </button>
                            </div>
                        {:else}
                            <p class="text-dark-400 text-sm">No flakes available. Create one in Admin → Flakes.</p>
                        {/if}
                    </div>

                    <div class="grid grid-cols-2 gap-4">
                        <div class="input-group">
                            <label for="daemon" class="input-label">Node</label>
                            <Select
                                id="daemon"
                                bind:value={newContainer.daemonId}
                                placeholder="Select a node..."
                                options={daemons.map(d => ({ value: d.id, label: `${d.name} (${d.host})` }))}
                                required
                            />
                        </div>

                        <div class="input-group">
                            <label for="name" class="input-label">Server Name</label>
                            <input type="text" id="name" bind:value={newContainer.name} class="input" placeholder="my-server" required />
                        </div>
                    </div>

                    <!-- Docker Image (only when no flake selected) -->
                    {#if !newContainer.flakeId}
                        <div class="input-group">
                            <label for="image" class="input-label">Docker Image</label>
                            <input
                                type="text"
                                id="image"
                                bind:value={newContainer.image}
                                class="input"
                                placeholder="artifacts.lstan.eu/java:21"
                                required
                            />
                        </div>
                    {/if}

                    <!-- Flake Variables (when flake selected) -->
                    {#if selectedFlake && selectedFlake.variables.length > 0}
                        <div class="input-group">
                            <label class="input-label">Server Variables</label>
                            <div class="space-y-3 bg-dark-800/50 p-3 rounded-lg border border-dark-700">
                                {#each selectedFlake.variables.filter(v => v.userViewable) as variable}
                                    <div>
                                        <label for="var-{variable.envVariable}" class="text-sm text-dark-300 flex items-center gap-2">
                                            {variable.name}
                                            <code class="text-xs text-primary-400">{variable.envVariable}</code>
                                        </label>
                                        {#if variable.description}
                                            <p class="text-xs text-dark-500 mb-1">{variable.description}</p>
                                        {/if}
                                        <input
                                            type="text"
                                            id="var-{variable.envVariable}"
                                            bind:value={flakeVariables[variable.envVariable]}
                                            class="input w-full"
                                            placeholder={variable.defaultValue || ''}
                                            disabled={!variable.userEditable}
                                        />
                                    </div>
                                {/each}
                            </div>
                        </div>
                    {/if}

                    <!-- Startup Command -->
                    <div class="input-group">
                        <label for="startup" class="input-label">
                            Startup Command
                            {#if selectedFlake}
                                <span class="text-dark-500 font-normal">(from flake, editable)</span>
                            {:else}
                                <span class="text-dark-500 font-normal">(optional)</span>
                            {/if}
                        </label>
                        <textarea
                            id="startup"
                            bind:value={newContainer.startupScript}
                            class="input min-h-[80px] resize-none font-mono text-sm"
                            placeholder={selectedFlake ? selectedFlake.startupCommand : "java -Xms128M -Xmx1024M -jar server.jar nogui"}
                            rows="2"
                        ></textarea>
                        {#if selectedFlake}
                            <p class="text-xs text-dark-500 mt-1">Variables like {"{{SERVER_MEMORY}}"} will be replaced with their values</p>
                        {/if}
                    </div>

                    <!-- Resource Limits -->
                    <div class="grid grid-cols-3 gap-4">
                        <div class="input-group">
                            <label for="memory" class="input-label">Memory (MB)</label>
                            <input type="number" id="memory" bind:value={newContainer.memoryLimit} class="input" min="128" step="128" required />
                        </div>
                        <div class="input-group">
                            <label for="cpu" class="input-label">CPU Cores</label>
                            <input type="number" id="cpu" bind:value={newContainer.cpuLimit} class="input" min="0.1" step="0.1" required />
                        </div>
                        <div class="input-group">
                            <label for="disk" class="input-label">Disk (MB)</label>
                            <input type="number" id="disk" bind:value={newContainer.diskLimit} class="input" min="512" step="512" required />
                        </div>
                    </div>


                    <div class="input-group">
                        <label for="allocation" class="input-label">
                            Network Allocation
                            <span class="text-dark-500 font-normal">(optional)</span>
                        </label>
                        <Select
                            id="allocation"
                            bind:value={newContainer.allocationId}
                            placeholder="Auto-assign"
                            options={[
                                { value: '', label: 'Auto-assign' },
                                ...availableAllocations.map(a => ({ value: a.id, label: `${a.ip}:${a.port}` }))
                            ]}
                        />
                    </div>

                    <!-- User Assignment (Admin only) -->
                    {#if isAdmin}
                        <div class="input-group">
                            <label for="user-assign" class="input-label">
                                Assign to User
                                <span class="text-dark-500 font-normal">(optional - defaults to you)</span>
                            </label>
                            <UserSearch
                                bind:value={newContainer.userId}
                                placeholder="Search users to assign..."
                                on:select={(e) => selectedUser = e.detail}
                            />
                            {#if selectedUser}
                                <p class="text-xs text-dark-400 mt-1">
                                    Server will be assigned to: <span class="text-primary-400">{selectedUser.username}</span>
                                </p>
                            {/if}
                        </div>
                    {/if}

                    <div class="flex gap-3 pt-4">
                        <button type="submit" class="btn-success flex-1" disabled={creating}>
                            {#if creating}
                                <span class="spinner"></span>
                                Creating...
                            {:else}
                                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                                </svg>
                                Create Server
                            {/if}
                        </button>
                        <button type="button" on:click={() => showCreate = false} class="btn-secondary">
                            Cancel
                        </button>
                    </div>
                </form>
            </div>
            </div>
        </div>
    {/if}

    {#if loading}
        <div class="flex items-center justify-center py-20">
            <div class="text-center">
                <div class="spinner w-8 h-8 mx-auto mb-4"></div>
                <p class="text-dark-400">Loading servers...</p>
            </div>
        </div>
    {:else}
        <!-- Table -->
        <div class="table-container">
            <table class="table">
                <thead>
                    <tr>
                        <th>Container</th>
                        <th>Address</th>
                        <th>Resources</th>
                        <th>Status</th>
                        <th class="text-right">Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {#each containers as container, i}
                        <tr class="animate-slide-up" style="animation-delay: {i * 30}ms;">
                            <td>
                                <div class="flex items-center gap-3">
                                    <div class="w-9 h-9 rounded-lg bg-gradient-to-br from-primary-500/20 to-primary-600/10 flex items-center justify-center">
                                        <svg class="w-4 h-4 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M21 7.5l-9-5.25L3 7.5m18 0l-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9" />
                                        </svg>
                                    </div>
                                    <div class="min-w-0">
                                        <a href="/containers/{container.id}" class="font-medium text-white hover:text-primary-400 transition-colors duration-200 block truncate max-w-[180px]" title={container.name}>
                                            {container.name}
                                        </a>
                                        <span class="text-xs text-dark-500 truncate block max-w-[180px]" title={container.image}>{container.image.split('/').pop()}</span>
                                    </div>
                                </div>
                            </td>
                            <td>
                                {#if container.allocationIp && container.allocationPort}
                                    <code class="text-sm bg-dark-800 px-2 py-1 rounded text-primary-400 font-mono">{container.allocationIp}:{container.allocationPort}</code>
                                {:else}
                                    <span class="text-dark-500 text-sm">No allocation</span>
                                {/if}
                            </td>
                            <td>
                                <div class="flex items-center gap-3 text-xs">
                                    <span class="text-dark-400" title="Memory">
                                        <svg class="w-3.5 h-3.5 inline mr-1 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z" />
                                        </svg>
                                        {container.memoryLimit || 1024} MB
                                    </span>
                                    <span class="text-dark-400" title="CPU">
                                        <svg class="w-3.5 h-3.5 inline mr-1 text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
                                        </svg>
                                        {container.cpuLimit || 1}x
                                    </span>
                                </div>
                            </td>
                            <td>
                                <span class={getStatusBadge(container.status)}>
                                    <span class="w-1.5 h-1.5 rounded-full {container.status.toLowerCase() === 'running' ? 'bg-emerald-400 animate-pulse' : 'bg-current'}"></span>
                                    {container.status}
                                </span>
                            </td>
                            <td class="text-right">
                                <div class="flex items-center justify-end gap-2">
                                    <a href="/containers/{container.id}" class="btn-ghost btn-sm">
                                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963 7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z" />
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                                        </svg>
                                        View
                                    </a>
                                    <button on:click={() => startDelete(container.id, container.name)} class="btn-ghost btn-sm text-red-400 hover:text-red-300 hover:bg-red-500/10">
                                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0" />
                                        </svg>
                                        Delete
                                    </button>
                                </div>
                            </td>
                        </tr>
                    {:else}
                        <tr>
                            <td colspan="5">
                                <div class="py-12 text-center">
                                    <div class="w-16 h-16 rounded-2xl bg-dark-800 flex items-center justify-center mx-auto mb-4">
                                        <svg class="w-8 h-8 text-dark-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M21 7.5l-9-5.25L3 7.5m18 0l-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9" />
                                        </svg>
                                    </div>
                                    <h3 class="text-lg font-semibold text-white mb-2">No containers found</h3>
                                    <p class="text-dark-400 mb-6">Create your first container to get started</p>
                                    <button on:click={() => showCreate = true} class="btn-primary inline-flex">
                                        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                                        </svg>
                                        Create Container
                                    </button>
                                </div>
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</div>

<!-- Delete Confirmation Modal -->
{#if deleteTarget}
    <div class="fixed inset-0 z-50 overflow-y-auto">
        <div
            class="fixed inset-0 bg-dark-950/80 backdrop-blur-sm"
            on:click={cancelDelete}
            on:keydown={(e) => e.key === 'Escape' && cancelDelete()}
            role="button"
            tabindex="-1"
        ></div>

        <div class="flex min-h-full items-center justify-center p-4">
            <div class="relative bg-dark-900 rounded-xl border border-dark-700 shadow-2xl w-full max-w-md p-6">
                <!-- Warning Icon -->
                <div class="w-16 h-16 mx-auto mb-4 rounded-full bg-red-500/10 flex items-center justify-center">
                    <svg class="w-8 h-8 text-red-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
                    </svg>
                </div>

                <!-- Step indicators -->
                <div class="flex justify-center gap-2 mb-4">
                    {#each [1, 2, 3] as step}
                        <div class="w-2 h-2 rounded-full transition-colors {deleteStep >= step ? 'bg-red-500' : 'bg-dark-600'}"></div>
                    {/each}
                </div>

                {#if deleteStep === 1}
                    <h3 class="text-xl font-bold text-white text-center mb-2">Delete Server?</h3>
                    <p class="text-dark-400 text-center mb-6">
                        You are about to delete <span class="text-white font-semibold">{deleteTarget.name}</span>. This action cannot be undone.
                    </p>
                {:else if deleteStep === 2}
                    <h3 class="text-xl font-bold text-white text-center mb-2">Are you absolutely sure?</h3>
                    <p class="text-dark-400 text-center mb-6">
                        All data, files, and configurations for this server will be <span class="text-red-400 font-semibold">permanently deleted</span>.
                    </p>
                {:else if deleteStep === 3}
                    <h3 class="text-xl font-bold text-white text-center mb-2">Final Confirmation</h3>
                    <p class="text-dark-400 text-center mb-4">
                        Type <span class="text-red-400 font-mono font-semibold">{deleteTarget.name}</span> to confirm deletion:
                    </p>
                    <input
                        type="text"
                        bind:value={deleteConfirmName}
                        placeholder="Enter server name"
                        class="input w-full mb-4 text-center"
                        autocomplete="off"
                    />
                {/if}

                <div class="flex gap-3">
                    <button on:click={cancelDelete} class="btn-ghost flex-1" disabled={deleting}>
                        Cancel
                    </button>
                    {#if deleteStep < 3}
                        <button on:click={confirmDelete} class="btn-danger flex-1">
                            {deleteStep === 1 ? 'Yes, Delete' : 'I Understand'}
                        </button>
                    {:else}
                        <button
                            on:click={confirmDelete}
                            class="btn-danger flex-1"
                            disabled={deleting || deleteConfirmName !== deleteTarget.name}
                        >
                            {#if deleting}
                                <span class="spinner w-4 h-4"></span>
                            {/if}
                            Delete Forever
                        </button>
                    {/if}
                </div>
            </div>
        </div>
    </div>
{/if}
