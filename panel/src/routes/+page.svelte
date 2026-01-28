<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '$lib/api';
    import { user } from '$lib/stores';
    import { goto } from '$app/navigation';
    import type { Container } from '$lib/types';

    let containers: Container[] = [];
    let loading = true;

    onMount(async () => {
        if (!$user) {
            goto('/login');
            return;
        }
        try {
            containers = await api.listContainers();
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    });

    function getStatusBadge(status: string) {
        switch (status.toLowerCase()) {
            case 'running': return 'badge-success';
            case 'stopped': return 'badge-danger';
            case 'starting': return 'badge-warning';
            default: return 'badge-neutral';
        }
    }

    $: runningCount = containers.filter(c => c.status.toLowerCase() === 'running').length;
    $: stoppedCount = containers.filter(c => c.status.toLowerCase() === 'stopped').length;
</script>

<div class="space-y-8">
    <!-- Header -->
    <div>
        <h1 class="section-title">Dashboard</h1>
        <p class="section-subtitle">Welcome back, {$user?.username}. Here's what's happening with your containers.</p>
    </div>

    {#if loading}
        <div class="flex items-center justify-center py-20">
            <div class="text-center">
                <div class="spinner w-8 h-8 mx-auto mb-4"></div>
                <p class="text-dark-400">Loading your dashboard...</p>
            </div>
        </div>
    {:else}
        <!-- Stats Grid -->
        <div class="grid gap-6 sm:grid-cols-2 lg:grid-cols-4">
            <!-- Total Containers -->
            <div class="card p-6 animate-slide-up" style="animation-delay: 0ms;">
                <div class="flex items-center justify-between">
                    <div>
                        <p class="text-dark-400 text-sm font-medium">Total Containers</p>
                        <p class="text-3xl font-bold text-white mt-1">{containers.length}</p>
                    </div>
                    <div class="w-12 h-12 rounded-xl bg-primary-500/10 flex items-center justify-center">
                        <svg class="w-6 h-6 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M21 7.5l-9-5.25L3 7.5m18 0l-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9" />
                        </svg>
                    </div>
                </div>
            </div>

            <!-- Running -->
            <div class="card p-6 animate-slide-up" style="animation-delay: 50ms;">
                <div class="flex items-center justify-between">
                    <div>
                        <p class="text-dark-400 text-sm font-medium">Running</p>
                        <p class="text-3xl font-bold text-emerald-400 mt-1">{runningCount}</p>
                    </div>
                    <div class="w-12 h-12 rounded-xl bg-emerald-500/10 flex items-center justify-center">
                        <svg class="w-6 h-6 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M5.25 5.653c0-.856.917-1.398 1.667-.986l11.54 6.348a1.125 1.125 0 010 1.971l-11.54 6.347a1.125 1.125 0 01-1.667-.985V5.653z" />
                        </svg>
                    </div>
                </div>
            </div>

            <!-- Stopped -->
            <div class="card p-6 animate-slide-up" style="animation-delay: 100ms;">
                <div class="flex items-center justify-between">
                    <div>
                        <p class="text-dark-400 text-sm font-medium">Stopped</p>
                        <p class="text-3xl font-bold text-red-400 mt-1">{stoppedCount}</p>
                    </div>
                    <div class="w-12 h-12 rounded-xl bg-red-500/10 flex items-center justify-center">
                        <svg class="w-6 h-6 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M5.25 7.5A2.25 2.25 0 017.5 5.25h9a2.25 2.25 0 012.25 2.25v9a2.25 2.25 0 01-2.25 2.25h-9a2.25 2.25 0 01-2.25-2.25v-9z" />
                        </svg>
                    </div>
                </div>
            </div>

            <!-- Quick Action -->
            <div class="card p-6 animate-slide-up" style="animation-delay: 150ms;">
                <div class="flex items-center justify-between">
                    <div>
                        <p class="text-dark-400 text-sm font-medium">Quick Action</p>
                        <a href="/containers" class="btn-primary btn-sm mt-3 inline-flex">
                            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                            </svg>
                            New Container
                        </a>
                    </div>
                    <div class="w-12 h-12 rounded-xl bg-amber-500/10 flex items-center justify-center">
                        <svg class="w-6 h-6 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 13.5l10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75z" />
                        </svg>
                    </div>
                </div>
            </div>
        </div>

        <!-- Containers Section -->
        <div>
            <div class="flex items-center justify-between mb-6">
                <h2 class="text-lg font-semibold text-white">Your Containers</h2>
                <a href="/containers" class="text-sm text-primary-400 hover:text-primary-300 transition-colors duration-200">
                    View all →
                </a>
            </div>

            <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                {#each containers as container, i}
                    <a
                        href="/containers/{container.id}"
                        class="card-hover p-5 group animate-slide-up"
                        style="animation-delay: {200 + i * 50}ms;"
                    >
                        <div class="flex items-start justify-between mb-4">
                            <div class="flex items-center gap-3">
                                <div class="w-10 h-10 rounded-lg bg-gradient-to-br from-primary-500/20 to-primary-600/10 flex items-center justify-center group-hover:from-primary-500/30 group-hover:to-primary-600/20 transition-all duration-300">
                                    <svg class="w-5 h-5 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M21 7.5l-9-5.25L3 7.5m18 0l-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9" />
                                    </svg>
                                </div>
                                <div>
                                    <h3 class="font-semibold text-white group-hover:text-primary-400 transition-colors duration-200">{container.name}</h3>
                                    <p class="text-xs text-dark-400 truncate max-w-[140px]">{container.image}</p>
                                </div>
                            </div>
                            <span class={getStatusBadge(container.status)}>
                                <span class="w-1.5 h-1.5 rounded-full {container.status.toLowerCase() === 'running' ? 'bg-emerald-400 animate-pulse' : 'bg-current'}"></span>
                                {container.status}
                            </span>
                        </div>
                        <div class="flex items-center gap-4 text-xs text-dark-500">
                            <span class="flex items-center gap-1">
                                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M6.75 3v2.25M17.25 3v2.25M3 18.75V7.5a2.25 2.25 0 012.25-2.25h13.5A2.25 2.25 0 0121 7.5v11.25m-18 0A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75m-18 0v-7.5A2.25 2.25 0 015.25 9h13.5A2.25 2.25 0 0121 11.25v7.5" />
                                </svg>
                                {new Date(container.createdAt).toLocaleDateString()}
                            </span>
                        </div>
                    </a>
                {:else}
                    <div class="col-span-full card p-12 text-center">
                        <div class="w-16 h-16 rounded-2xl bg-dark-800 flex items-center justify-center mx-auto mb-4">
                            <svg class="w-8 h-8 text-dark-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M21 7.5l-9-5.25L3 7.5m18 0l-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9" />
                            </svg>
                        </div>
                        <h3 class="text-lg font-semibold text-white mb-2">No containers yet</h3>
                        <p class="text-dark-400 mb-6">Get started by creating your first container</p>
                        <a href="/containers" class="btn-primary inline-flex">
                            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                            </svg>
                            Create Container
                        </a>
                    </div>
                {/each}
            </div>
        </div>
    {/if}
</div>
