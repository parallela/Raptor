<script lang="ts">
    import { page } from '$app/stores';
    import { getContext, onMount } from 'svelte';
    import type { Writable } from 'svelte/store';
    import { api } from '$lib/api';
    import toast from 'svelte-french-toast';
    import type { Container, ContainerAllocation, Allocation } from '$lib/types';

    const containerStore = getContext<Writable<Container | null>>('container');
    const allocationsStore = getContext<Writable<ContainerAllocation[]>>('allocations');
    const availableAllocationsStore = getContext<Writable<Allocation[]>>('availableAllocations');
    const actionLoadingStore = getContext<Writable<string>>('actionLoading');
    const actions = getContext<any>('actions');

    let editMemory = 0;
    let editServerMemory = 0;
    let editCpu = 0;
    let editDisk = 0;
    let editSwap = 0;
    let savingSettings = false;
    let loadingAllocations = false;

    let showKillModal = false;
    let showAddAllocationModal = false;
    let addingAllocation = false;
    let settingPrimary = '';
    let copiedField: string | null = null;

    $: containerId = $page.params.id as string;
    $: container = $containerStore;
    $: containerAllocations = $allocationsStore;
    $: availableAllocations = $availableAllocationsStore;
    $: actionLoading = $actionLoadingStore;

    $: if (container && editMemory === 0) {
        editMemory = container.memoryLimit || 1024;
        editServerMemory = (container as any).serverMemory || container.memoryLimit || 1024;
        editCpu = container.cpuLimit || 1;
        editDisk = container.diskLimit || 5120;
        editSwap = container.swapLimit || 0;
    }

    onMount(async () => {
        await loadAllocations();
    });

    async function loadAllocations() {
        if (!container?.daemonId) return;
        loadingAllocations = true;
        try {
            await actions.loadAllocations();
        } catch (e) {
            console.error('Failed to load allocations', e);
        } finally {
            loadingAllocations = false;
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
                swapLimit: editSwap
            });
            toast.success('Settings saved');
            await actions.loadContainer();
        } catch (e: any) {
            toast.error(e.message || 'Failed to save settings');
        } finally {
            savingSettings = false;
        }
    }

    async function killContainer() {
        await actions.killContainer();
        showKillModal = false;
    }

    async function openAddAllocationModal() {
        await loadAllocations();
        showAddAllocationModal = true;
    }

    async function addAllocation(allocationId: string) {
        addingAllocation = true;
        try {
            await api.addContainerAllocation(containerId, allocationId);
            toast.success('Allocation added');
            await loadAllocations();
            showAddAllocationModal = false;
        } catch (e: any) {
            toast.error(e.message || 'Failed to add allocation');
        } finally {
            addingAllocation = false;
        }
    }

    async function removeAllocation(allocationId: string) {
        try {
            await api.removeContainerAllocation(containerId, allocationId);
            toast.success('Allocation removed');
            await loadAllocations();
        } catch (e: any) {
            toast.error(e.message || 'Failed to remove allocation');
        }
    }

    async function setAsPrimary(allocationId: string) {
        settingPrimary = allocationId;
        try {
            await api.setContainerPrimaryAllocation(containerId, allocationId);
            toast.success('Primary allocation set');
            await loadAllocations();
            await actions.loadContainer();
        } catch (e: any) {
            toast.error(e.message || 'Failed to set primary allocation');
        } finally {
            settingPrimary = '';
        }
    }

    function copyToClipboard(text: string, field: string) {
        navigator.clipboard.writeText(text);
        copiedField = field;
        toast.success('Copied to clipboard');
        setTimeout(() => copiedField = null, 2000);
    }
</script>

<div class="p-3 md:p-6 max-w-2xl overflow-y-auto h-full">
    {#if container}
        <h2 class="text-base md:text-lg font-semibold text-white mb-3 md:mb-4">Server Settings</h2>
        <div class="space-y-3 md:space-y-4">
            <!-- Server Information -->
            <div class="card p-3 md:p-4">
                <h3 class="text-xs md:text-sm font-medium text-dark-400 mb-2 md:mb-3">Server Information</h3>
                <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 md:gap-4">
                    <div><span class="text-dark-400 text-xs md:text-sm">Name</span><p class="text-white font-medium truncate text-sm md:text-base">{container.name}</p></div>
                    <div><span class="text-dark-400 text-xs md:text-sm">Image</span><p class="text-white font-mono text-xs truncate">{container.image}</p></div>
                    <div><span class="text-dark-400 text-xs md:text-sm">Container ID</span><p class="text-white font-mono text-xs truncate">{container.id}</p></div>
                    <div><span class="text-dark-400 text-xs md:text-sm">Created</span><p class="text-white text-sm">{new Date(container.createdAt).toLocaleDateString()}</p></div>
                </div>
            </div>

            <!-- Network / Allocation Info -->
            <div class="card p-3 md:p-4">
                <div class="flex items-center justify-between mb-2 md:mb-3">
                    <h3 class="text-xs md:text-sm font-medium text-dark-400">Network Allocations</h3>
                    <button on:click={openAddAllocationModal} class="btn-secondary text-xs py-1 px-2">
                        <svg class="w-3 h-3 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4" /></svg>
                        <span class="hidden sm:inline">Add Allocation</span>
                        <span class="sm:hidden">Add</span>
                    </button>
                </div>

                {#if loadingAllocations}
                    <div class="text-center py-4"><span class="spinner w-5 h-5"></span></div>
                {:else if containerAllocations.length === 0}
                    <p class="text-dark-500 text-xs md:text-sm">No allocations assigned</p>
                {:else}
                    <div class="space-y-2">
                        {#each containerAllocations as alloc}
                            <div class="flex flex-col sm:flex-row sm:items-center justify-between bg-dark-900/50 rounded-lg p-2 md:p-3 gap-2">
                                <div class="flex items-center gap-2 flex-wrap">
                                    <code class="text-primary-400 font-mono text-xs md:text-sm">{alloc.ip}:{alloc.port}</code>
                                    <span class="text-xs font-mono px-1.5 py-0.5 rounded {
                                        alloc.protocol === 'both'
                                            ? 'bg-purple-500/20 text-purple-400'
                                            : alloc.protocol === 'udp'
                                                ? 'bg-amber-500/20 text-amber-400'
                                                : 'bg-blue-500/20 text-blue-400'
                                    }">
                                        {alloc.protocol === 'both' ? 'TCP+UDP' : alloc.protocol}
                                    </span>
                                    {#if alloc.isPrimary}
                                        <span class="text-xs bg-primary-500/20 text-primary-400 px-2 py-0.5 rounded">Primary</span>
                                    {/if}
                                </div>
                                <div class="flex items-center gap-1 sm:gap-2">
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
                                    <button on:click={() => copyToClipboard(`${alloc.ip}:${alloc.port}`, `alloc-${alloc.id}`)} class="text-dark-400 hover:text-white p-1">
                                        {#if copiedField === `alloc-${alloc.id}`}
                                            <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
                                        {:else}
                                            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" /></svg>
                                        {/if}
                                    </button>
                                    <button
                                        on:click={() => alloc.allocationId && removeAllocation(alloc.allocationId)}
                                        class="text-dark-400 hover:text-red-400 p-1"
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
            <div class="card p-3 md:p-4">
                <h3 class="text-xs md:text-sm font-medium text-dark-400 mb-2 md:mb-3">Resource Limits</h3>
                <form on:submit|preventDefault={saveSettings} class="space-y-3 md:space-y-4">
                    <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 md:gap-4">
                        <div>
                            <label for="serverMemory" class="text-dark-400 text-xs md:text-sm block mb-1">Server Memory (MB)</label>
                            <input id="serverMemory" type="number" bind:value={editServerMemory} min="128" class="input w-full text-sm" placeholder="e.g., 1024" />
                            <p class="text-dark-500 text-xs mt-1">JVM heap memory (-Xmx)</p>
                        </div>
                        <div>
                            <label for="containerMemory" class="text-dark-400 text-xs md:text-sm block mb-1">Container Memory (MB)</label>
                            <input id="containerMemory" type="number" bind:value={editMemory} min="128" class="input w-full text-sm" placeholder="e.g., 1280" />
                            <p class="text-dark-500 text-xs mt-1">Docker limit (~20% higher)</p>
                        </div>
                        <div>
                            <label for="cpuLimit" class="text-dark-400 text-xs md:text-sm block mb-1">CPU Limit (cores)</label>
                            <input id="cpuLimit" type="number" bind:value={editCpu} min="1" step="1" class="input w-full text-sm" placeholder="e.g., 1" />
                        </div>
                        <div>
                            <label for="diskSpace" class="text-dark-400 text-xs md:text-sm block mb-1">Disk Space (MB)</label>
                            <input id="diskSpace" type="number" bind:value={editDisk} min="1024" class="input w-full text-sm" placeholder="e.g., 5120" />
                        </div>
                        <div>
                            <label for="swapLimit" class="text-dark-400 text-xs md:text-sm block mb-1">Swap (MB)</label>
                            <input id="swapLimit" type="number" bind:value={editSwap} min="0" class="input w-full text-sm" placeholder="e.g., 0" />
                        </div>
                    </div>
                    <div class="flex justify-end">
                        <button type="submit" class="btn-primary text-sm" disabled={savingSettings}>
                            {#if savingSettings}<span class="spinner w-4 h-4 mr-2"></span>{/if}
                            Save Changes
                        </button>
                    </div>
                </form>
                <p class="text-dark-500 text-xs mt-2 md:mt-3">Note: Resource changes take effect after restarting the server.</p>
            </div>

            <!-- Danger Zone -->
            <div class="card p-3 md:p-4 border-red-500/20">
                <h3 class="text-xs md:text-sm font-medium text-red-400 mb-2 md:mb-3">Danger Zone</h3>
                <p class="text-dark-400 text-xs md:text-sm mb-2 md:mb-3">Force kill the server if it's unresponsive.</p>
                <button on:click={() => showKillModal = true} class="btn-danger text-sm">Kill Server</button>
            </div>
        </div>
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
