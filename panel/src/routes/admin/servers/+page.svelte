<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '$lib/api';
    import { user } from '$lib/stores';
    import { Select } from '$lib/components';
    import { goto } from '$app/navigation';
    import toast from 'svelte-french-toast';
    import type { Daemon, Allocation, User } from '$lib/types';

    let daemons: Daemon[] = [];
    let allocations: Allocation[] = [];
    let users: User[] = [];
    let loading = true;
    let showCreate = false;
    let creating = false;
    let error = '';

    // Server configuration
    let newServer = {
        name: '',
        daemonId: '',
        image: '',
        startupScript: '',
        allocationId: '',
        userId: '',
        // Resource limits
        memoryLimit: 1280,
        serverMemory: 1024,
        cpuLimit: 1,
        diskLimit: 10240,
        swapLimit: 512,
        ioWeight: 500,
    };

    // Presets for common server types
    const presets = [
        { name: 'Minecraft (Basic)', image: 'itzg/minecraft-server', memory: 2048, cpu: 1, disk: 10240 },
        { name: 'Minecraft (Performance)', image: 'itzg/minecraft-server', memory: 4096, cpu: 2, disk: 20480 },
        { name: 'Node.js App', image: 'node:20-alpine', memory: 512, cpu: 0.5, disk: 5120 },
        { name: 'Python App', image: 'python:3.12-slim', memory: 512, cpu: 0.5, disk: 5120 },
        { name: 'Custom', image: '', memory: 1024, cpu: 1, disk: 10240 },
    ];

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
            [daemons, allocations] = await Promise.all([
                api.listDaemons(),
                api.listAllocations(),
            ]);
            try {
                users = await api.listUsers();
            } catch {
                users = [];
            }
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    }

    function applyPreset(preset: typeof presets[0]) {
        newServer.image = preset.image;
        newServer.serverMemory = preset.memory;
        newServer.memoryLimit = Math.round(preset.memory * 1.25); // 25% overhead for container
        newServer.cpuLimit = preset.cpu;
        newServer.diskLimit = preset.disk;
    }

    async function createServer() {
        error = '';
        creating = true;
        try {
            // Validate required allocation
            if (!newServer.allocationId) {
                error = 'Please select a network allocation';
                toast.error(error);
                creating = false;
                return;
            }

            await api.createContainer({
                name: newServer.name,
                daemonId: newServer.daemonId,
                image: newServer.image,
                startupScript: newServer.startupScript || undefined,
                allocationId: newServer.allocationId,
                userId: newServer.userId || undefined,
                memoryLimit: newServer.memoryLimit,
                serverMemory: newServer.serverMemory,
                cpuLimit: newServer.cpuLimit,
                diskLimit: newServer.diskLimit,
                swapLimit: newServer.swapLimit,
                ioWeight: newServer.ioWeight,
            });
            toast.success('Server created successfully');
            goto('/admin');
        } catch (e: any) {
            error = e.message || 'Failed to create server';
            toast.error(error);
        } finally {
            creating = false;
        }
    }

    $: availableAllocations = allocations.filter(a => !a.containerId && a.daemonId === newServer.daemonId);
    $: selectedDaemon = daemons.find(d => d.id === newServer.daemonId);
</script>

<div class="space-y-6 max-w-4xl mx-auto">
    <!-- Header -->
    <div class="flex items-center gap-4">
        <a href="/admin" class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-800 transition-colors duration-200">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" />
            </svg>
        </a>
        <div>
            <h1 class="section-title">Create New Server</h1>
            <p class="section-subtitle">Configure resources and deploy a new container</p>
        </div>
    </div>

    {#if error}
        <div class="flex items-center gap-3 p-4 rounded-lg bg-red-500/10 border border-red-500/20 animate-slide-down">
            <svg class="w-5 h-5 text-red-400 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
            </svg>
            <span class="text-red-400 text-sm">{error}</span>
        </div>
    {/if}

    {#if loading}
        <div class="flex items-center justify-center py-20">
            <div class="text-center">
                <div class="spinner w-8 h-8 mx-auto mb-4"></div>
                <p class="text-dark-400">Loading...</p>
            </div>
        </div>
    {:else}
        <form on:submit|preventDefault={createServer} class="space-y-6">
            <!-- Presets -->
            <div class="card p-6">
                <h2 class="text-lg font-semibold text-white mb-4 flex items-center gap-2">
                    <svg class="w-5 h-5 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09zM18.259 8.715L18 9.75l-.259-1.035a3.375 3.375 0 00-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 002.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 002.456 2.456L21.75 6l-1.035.259a3.375 3.375 0 00-2.456 2.456zM16.894 20.567L16.5 21.75l-.394-1.183a2.25 2.25 0 00-1.423-1.423L13.5 18.75l1.183-.394a2.25 2.25 0 001.423-1.423l.394-1.183.394 1.183a2.25 2.25 0 001.423 1.423l1.183.394-1.183.394a2.25 2.25 0 00-1.423 1.423z" />
                    </svg>
                    Quick Presets
                </h2>
                <div class="grid grid-cols-2 md:grid-cols-5 gap-3">
                    {#each presets as preset}
                        <button
                            type="button"
                            on:click={() => applyPreset(preset)}
                            class="p-3 rounded-lg border border-dark-600/50 hover:border-primary-500/50 hover:bg-dark-800/50 transition-all duration-200 text-left"
                        >
                            <p class="text-sm font-medium text-white">{preset.name}</p>
                            <p class="text-xs text-dark-400 mt-1">{preset.memory}MB / {preset.cpu} CPU</p>
                        </button>
                    {/each}
                </div>
            </div>

            <!-- Basic Info -->
            <div class="card p-6">
                <h2 class="text-lg font-semibold text-white mb-4 flex items-center gap-2">
                    <svg class="w-5 h-5 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M11.25 11.25l.041-.02a.75.75 0 011.063.852l-.708 2.836a.75.75 0 001.063.853l.041-.021M21 12a9 9 0 11-18 0 9 9 0 0118 0zm-9-3.75h.008v.008H12V8.25z" />
                    </svg>
                    Basic Information
                </h2>
                <div class="grid gap-5 md:grid-cols-2">
                    <div class="input-group">
                        <label for="name" class="input-label">Server Name</label>
                        <input type="text" id="name" bind:value={newServer.name} class="input" placeholder="my-awesome-server" required />
                    </div>

                    <div class="input-group">
                        <label for="daemon" class="input-label">Node / Daemon</label>
                        <Select
                            id="daemon"
                            bind:value={newServer.daemonId}
                            placeholder="Select a node..."
                            options={daemons.map(d => ({
                                value: d.id,
                                label: `${d.name} (${d.host})${d.location ? ` — ${d.location}` : ''}`
                            }))}
                            required
                        />
                    </div>

                    <div class="input-group">
                        <label for="image" class="input-label">Docker Image</label>
                        <input type="text" id="image" bind:value={newServer.image} class="input" placeholder="e.g. itzg/minecraft-server" required />
                    </div>

                    <div class="input-group">
                        <label for="allocation" class="input-label">
                            Network Allocation
                            <span class="text-red-400 font-normal">*</span>
                        </label>
                        <Select
                            id="allocation"
                            bind:value={newServer.allocationId}
                            placeholder="Select an allocation..."
                            options={availableAllocations.map(a => ({ value: a.id, label: `${a.ip}:${a.port}` }))}
                            disabled={!newServer.daemonId}
                            required
                        />
                        {#if newServer.daemonId && availableAllocations.length === 0}
                            <p class="text-xs text-amber-400 mt-1">No allocations available for this daemon.</p>
                        {/if}
                    </div>

                    {#if users.length > 0}
                        <div class="input-group md:col-span-2">
                            <label for="user" class="input-label">
                                Assign to User
                                <span class="text-dark-500 font-normal">(optional)</span>
                            </label>
                            <Select
                                id="user"
                                bind:value={newServer.userId}
                                placeholder="Current user (you)"
                                options={[
                                    { value: '', label: 'Current user (you)' },
                                    ...users.map(u => ({ value: u.id, label: u.username }))
                                ]}
                            />
                        </div>
                    {/if}

                    <div class="input-group md:col-span-2">
                        <label for="startup" class="input-label">
                            Startup Command
                            <span class="text-dark-500 font-normal">(optional)</span>
                        </label>
                        <textarea
                            id="startup"
                            bind:value={newServer.startupScript}
                            class="input min-h-[80px] resize-none font-mono text-sm"
                            placeholder="Custom startup command or script..."
                            rows="3"
                        ></textarea>
                    </div>
                </div>
            </div>

            <!-- Resource Limits -->
            <div class="card p-6">
                <h2 class="text-lg font-semibold text-white mb-4 flex items-center gap-2">
                    <svg class="w-5 h-5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M8.25 3v1.5M4.5 8.25H3m18 0h-1.5M4.5 12H3m18 0h-1.5m-15 3.75H3m18 0h-1.5M8.25 19.5V21M12 3v1.5m0 15V21m3.75-18v1.5m0 15V21m-9-1.5h10.5a2.25 2.25 0 002.25-2.25V6.75a2.25 2.25 0 00-2.25-2.25H6.75A2.25 2.25 0 004.5 6.75v10.5a2.25 2.25 0 002.25 2.25zm.75-12h9v9h-9v-9z" />
                    </svg>
                    Resource Limits
                </h2>

                <div class="grid gap-6 md:grid-cols-2">
                    <!-- Server Memory (JVM Heap) -->
                    <div class="input-group">
                        <label for="serverMemory" class="input-label flex items-center justify-between">
                            <span>Server Memory</span>
                            <span class="text-primary-400 font-mono">{newServer.serverMemory} MB</span>
                        </label>
                        <input
                            type="range"
                            id="serverMemory"
                            bind:value={newServer.serverMemory}
                            min="128"
                            max="16384"
                            step="128"
                            class="w-full h-2 bg-dark-700 rounded-lg appearance-none cursor-pointer accent-primary-500"
                        />
                        <div class="flex justify-between text-xs text-dark-500 mt-1">
                            <span>128 MB</span>
                            <span>JVM heap (-Xmx)</span>
                        </div>
                    </div>

                    <!-- Container Memory (Docker limit) -->
                    <div class="input-group">
                        <label for="memory" class="input-label flex items-center justify-between">
                            <span>Container Memory</span>
                            <span class="text-primary-400 font-mono">{newServer.memoryLimit} MB</span>
                        </label>
                        <input
                            type="range"
                            id="memory"
                            bind:value={newServer.memoryLimit}
                            min="128"
                            max="20480"
                            step="128"
                            class="w-full h-2 bg-dark-700 rounded-lg appearance-none cursor-pointer accent-primary-500"
                        />
                        <div class="flex justify-between text-xs text-dark-500 mt-1">
                            <span>128 MB</span>
                            <span>Docker limit (~20% higher)</span>
                        </div>
                    </div>

                    <!-- CPU -->
                    <div class="input-group">
                        <label for="cpu" class="input-label flex items-center justify-between">
                            <span>CPU Cores</span>
                            <span class="text-primary-400 font-mono">{newServer.cpuLimit} cores</span>
                        </label>
                        <input
                            type="range"
                            id="cpu"
                            bind:value={newServer.cpuLimit}
                            min="0.25"
                            max="8"
                            step="0.25"
                            class="w-full h-2 bg-dark-700 rounded-lg appearance-none cursor-pointer accent-primary-500"
                        />
                        <div class="flex justify-between text-xs text-dark-500 mt-1">
                            <span>0.25</span>
                            <span>8 cores</span>
                        </div>
                    </div>

                    <!-- Disk -->
                    <div class="input-group">
                        <label for="disk" class="input-label flex items-center justify-between">
                            <span>Disk Space</span>
                            <span class="text-primary-400 font-mono">{(newServer.diskLimit / 1024).toFixed(1)} GB</span>
                        </label>
                        <input
                            type="range"
                            id="disk"
                            bind:value={newServer.diskLimit}
                            min="1024"
                            max="102400"
                            step="1024"
                            class="w-full h-2 bg-dark-700 rounded-lg appearance-none cursor-pointer accent-primary-500"
                        />
                        <div class="flex justify-between text-xs text-dark-500 mt-1">
                            <span>1 GB</span>
                            <span>100 GB</span>
                        </div>
                    </div>

                    <!-- Swap -->
                    <div class="input-group">
                        <label for="swap" class="input-label flex items-center justify-between">
                            <span>Swap Memory</span>
                            <span class="text-primary-400 font-mono">{newServer.swapLimit} MB</span>
                        </label>
                        <input
                            type="range"
                            id="swap"
                            bind:value={newServer.swapLimit}
                            min="0"
                            max="4096"
                            step="128"
                            class="w-full h-2 bg-dark-700 rounded-lg appearance-none cursor-pointer accent-primary-500"
                        />
                        <div class="flex justify-between text-xs text-dark-500 mt-1">
                            <span>0 MB</span>
                            <span>4 GB</span>
                        </div>
                    </div>
                </div>

                <!-- Resource Summary -->
                <div class="mt-6 p-4 rounded-lg bg-dark-800/50 border border-dark-700/50">
                    <h3 class="text-sm font-medium text-dark-300 mb-3">Resource Summary</h3>
                    <div class="grid grid-cols-5 gap-4">
                        <div class="text-center">
                            <p class="text-2xl font-bold text-white">{newServer.serverMemory}</p>
                            <p class="text-xs text-dark-400">MB Server</p>
                        </div>
                        <div class="text-center">
                            <p class="text-2xl font-bold text-white">{newServer.memoryLimit}</p>
                            <p class="text-xs text-dark-400">MB Container</p>
                        </div>
                        <div class="text-center">
                            <p class="text-2xl font-bold text-white">{newServer.cpuLimit}</p>
                            <p class="text-xs text-dark-400">CPU Cores</p>
                        </div>
                        <div class="text-center">
                            <p class="text-2xl font-bold text-white">{(newServer.diskLimit / 1024).toFixed(1)}</p>
                            <p class="text-xs text-dark-400">GB Disk</p>
                        </div>
                        <div class="text-center">
                            <p class="text-2xl font-bold text-white">{newServer.swapLimit}</p>
                            <p class="text-xs text-dark-400">MB Swap</p>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Submit -->
            <div class="flex gap-4">
                <button type="submit" class="btn-success flex-1" disabled={creating}>
                    {#if creating}
                        <span class="spinner"></span>
                        Creating Server...
                    {:else}
                        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M5.25 5.653c0-.856.917-1.398 1.667-.986l11.54 6.348a1.125 1.125 0 010 1.971l-11.54 6.347a1.125 1.125 0 01-1.667-.985V5.653z" />
                        </svg>
                        Create & Deploy Server
                    {/if}
                </button>
                <a href="/admin" class="btn-secondary">Cancel</a>
            </div>
        </form>
    {/if}
</div>
