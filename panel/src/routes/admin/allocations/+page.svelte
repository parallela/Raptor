<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '$lib/api';
    import { user } from '$lib/stores';
    import { Select } from '$lib/components';
    import { goto } from '$app/navigation';
    import toast from 'svelte-french-toast';
    import type { Allocation, Daemon } from '$lib/types';

    let allocations: Allocation[] = [];
    let daemons: Daemon[] = [];
    let loading = true;
    let showCreate = false;
    let creating = false;

    let newAllocation = {
        daemonId: '',
        ip: '',
        port: 25565,
    };

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
            [allocations, daemons] = await Promise.all([
                api.listAllAllocations(),
                api.listDaemons(),
            ]);
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    }

    async function createAllocation() {
        creating = true;
        try {
            await api.createAllocation({
                daemonId: newAllocation.daemonId,
                ip: newAllocation.ip,
                port: newAllocation.port,
            });
            showCreate = false;
            newAllocation = { daemonId: '', ip: '', port: 25565 };
            await loadData();
            toast.success('Allocation created successfully');
        } catch (e: any) {
            toast.error(e.message || 'Failed to create allocation');
        } finally {
            creating = false;
        }
    }

    async function deleteAllocation(id: string) {
        if (!confirm('Are you sure you want to delete this allocation?')) return;
        try {
            await api.deleteAllocation(id);
            await loadData();
            toast.success('Allocation deleted successfully');
        } catch (e: any) {
            toast.error(e.message || 'Failed to delete allocation');
        }
    }

    function getDaemonName(daemonId: string): string {
        const daemon = daemons.find(d => d.id === daemonId);
        return daemon ? daemon.name : 'Unknown';
    }
</script>

<div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
        <div class="flex items-center gap-4">
            <a href="/admin" class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-800 transition-colors duration-200">
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" />
                </svg>
            </a>
            <div>
                <h1 class="section-title">Network Allocations</h1>
                <p class="section-subtitle">Manage IP addresses and port assignments</p>
            </div>
        </div>
        <button on:click={() => showCreate = true} class="btn-primary">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
            Add Allocation
        </button>
    </div>

    <!-- Create Modal -->
    {#if showCreate}
        <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
            <div
                class="absolute inset-0 bg-dark-950/80 backdrop-blur-sm"
                on:click={() => showCreate = false}
                on:keydown={(e) => e.key === 'Escape' && (showCreate = false)}
                role="button"
                tabindex="-1"
            ></div>

            <div class="relative w-full max-w-lg card p-6 animate-slide-up">
                <div class="flex items-center justify-between mb-6">
                    <h2 class="text-xl font-semibold text-white">Add New Allocation</h2>
                    <button
                        on:click={() => showCreate = false}
                        class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-700/50 transition-colors duration-200"
                    >
                        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                        </svg>
                    </button>
                </div>

                <form on:submit|preventDefault={createAllocation} class="space-y-5">
                    <div class="input-group">
                        <label for="daemon" class="input-label">Node / Daemon</label>
                        <Select
                            id="daemon"
                            bind:value={newAllocation.daemonId}
                            placeholder="Select a node..."
                            options={daemons.map(d => ({ value: d.id, label: `${d.name} (${d.host})` }))}
                            required
                        />
                    </div>

                    <div class="grid grid-cols-2 gap-4">
                        <div class="input-group">
                            <label for="ip" class="input-label">IP Address</label>
                            <input type="text" id="ip" bind:value={newAllocation.ip} class="input font-mono" placeholder="0.0.0.0" required />
                        </div>
                        <div class="input-group">
                            <label for="port" class="input-label">Port</label>
                            <input type="number" id="port" bind:value={newAllocation.port} class="input font-mono" min="1" max="65535" required />
                        </div>
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
                                Add Allocation
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
                <p class="text-dark-400">Loading allocations...</p>
            </div>
        </div>
    {:else}
        <div class="table-container">
            <table class="table">
                <thead>
                    <tr>
                        <th>Address</th>
                        <th>Node</th>
                        <th>Status</th>
                        <th class="text-right">Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {#each allocations as alloc, i}
                        <tr class="animate-slide-up" style="animation-delay: {i * 30}ms;">
                            <td>
                                <div class="flex items-center gap-3">
                                    <div class="w-9 h-9 rounded-lg bg-gradient-to-br from-emerald-500/20 to-emerald-600/10 flex items-center justify-center">
                                        <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 21a9.004 9.004 0 008.716-6.747M12 21a9.004 9.004 0 01-8.716-6.747M12 21c2.485 0 4.5-4.03 4.5-9S14.485 3 12 3m0 18c-2.485 0-4.5-4.03-4.5-9S9.515 3 12 3m0 0a8.997 8.997 0 017.843 4.582M12 3a8.997 8.997 0 00-7.843 4.582m15.686 0A11.953 11.953 0 0112 10.5c-2.998 0-5.74-1.1-7.843-2.918m15.686 0A8.959 8.959 0 0121 12c0 .778-.099 1.533-.284 2.253m0 0A17.919 17.919 0 0112 16.5c-3.162 0-6.133-.815-8.716-2.247m0 0A9.015 9.015 0 013 12c0-1.605.42-3.113 1.157-4.418" />
                                        </svg>
                                    </div>
                                    <code class="text-sm font-mono text-white">{alloc.ip}:{alloc.port}</code>
                                </div>
                            </td>
                            <td class="text-dark-300">{getDaemonName(alloc.daemonId)}</td>
                            <td>
                                {#if alloc.containerId}
                                    <span class="badge-warning">
                                        <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
                                        </svg>
                                        In Use
                                    </span>
                                {:else}
                                    <span class="badge-success">
                                        <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
                                        Available
                                    </span>
                                {/if}
                            </td>
                            <td class="text-right">
                                <button
                                    on:click={() => deleteAllocation(alloc.id)}
                                    class="btn-ghost btn-sm text-red-400 hover:text-red-300 hover:bg-red-500/10"
                                    disabled={!!alloc.containerId}
                                    title={alloc.containerId ? 'Cannot delete allocation in use' : 'Delete allocation'}
                                >
                                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0" />
                                    </svg>
                                    Delete
                                </button>
                            </td>
                        </tr>
                    {:else}
                        <tr>
                            <td colspan="4">
                                <div class="py-12 text-center">
                                    <div class="w-16 h-16 rounded-2xl bg-dark-800 flex items-center justify-center mx-auto mb-4">
                                        <svg class="w-8 h-8 text-dark-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 21a9.004 9.004 0 008.716-6.747M12 21a9.004 9.004 0 01-8.716-6.747M12 21c2.485 0 4.5-4.03 4.5-9S14.485 3 12 3m0 18c-2.485 0-4.5-4.03-4.5-9S9.515 3 12 3m0 0a8.997 8.997 0 017.843 4.582M12 3a8.997 8.997 0 00-7.843 4.582m15.686 0A11.953 11.953 0 0112 10.5c-2.998 0-5.74-1.1-7.843-2.918m15.686 0A8.959 8.959 0 0121 12c0 .778-.099 1.533-.284 2.253m0 0A17.919 17.919 0 0112 16.5c-3.162 0-6.133-.815-8.716-2.247m0 0A9.015 9.015 0 013 12c0-1.605.42-3.113 1.157-4.418" />
                                        </svg>
                                    </div>
                                    <h3 class="text-lg font-semibold text-white mb-2">No allocations found</h3>
                                    <p class="text-dark-400 mb-6">Add IP and port allocations for your servers</p>
                                    <button on:click={() => showCreate = true} class="btn-primary inline-flex">
                                        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                                        </svg>
                                        Add Allocation
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
