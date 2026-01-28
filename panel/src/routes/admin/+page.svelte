<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '$lib/api';
    import { user } from '$lib/stores';
    import { goto } from '$app/navigation';
    import type { Container, Daemon, User } from '$lib/types';

    let containers: Container[] = [];
    let daemons: Daemon[] = [];
    let users: User[] = [];
    let loading = true;

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
            [containers, daemons] = await Promise.all([
                api.listAllContainers().catch(() => api.listContainers()),
                api.listDaemons(),
            ]);
            // Try to load users (admin only)
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

    $: runningContainers = containers.filter(c => c.status.toLowerCase() === 'running').length;
    $: totalMemory = daemons.reduce((sum, d) => sum + (d.totalMemory || 0), 0);
    $: usedMemory = daemons.reduce((sum, d) => sum + (d.usedMemory || 0), 0);
    $: totalCpu = daemons.reduce((sum, d) => sum + (d.totalCpu || 0), 0);
    $: usedCpu = daemons.reduce((sum, d) => sum + (d.usedCpu || 0), 0);
</script>

<div class="space-y-8">
    <!-- Header -->
    <div>
        <h1 class="section-title">Admin Dashboard</h1>
        <p class="section-subtitle">Manage servers, users, and system resources</p>
    </div>

    {#if loading}
        <div class="flex items-center justify-center py-20">
            <div class="text-center">
                <div class="spinner w-8 h-8 mx-auto mb-4"></div>
                <p class="text-dark-400">Loading admin data...</p>
            </div>
        </div>
    {:else}
        <!-- Stats Grid -->
        <div class="grid gap-6 sm:grid-cols-2 lg:grid-cols-4">
            <!-- Total Servers -->
            <div class="card p-6 animate-slide-up">
                <div class="flex items-center justify-between">
                    <div>
                        <p class="text-dark-400 text-sm font-medium">Total Servers</p>
                        <p class="text-3xl font-bold text-white mt-1">{containers.length}</p>
                        <p class="text-xs text-emerald-400 mt-1">{runningContainers} running</p>
                    </div>
                    <div class="w-12 h-12 rounded-xl bg-primary-500/10 flex items-center justify-center">
                        <svg class="w-6 h-6 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M21 7.5l-9-5.25L3 7.5m18 0l-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9" />
                        </svg>
                    </div>
                </div>
            </div>

            <!-- Nodes -->
            <div class="card p-6 animate-slide-up" style="animation-delay: 50ms;">
                <div class="flex items-center justify-between">
                    <div>
                        <p class="text-dark-400 text-sm font-medium">Nodes</p>
                        <p class="text-3xl font-bold text-white mt-1">{daemons.length}</p>
                        <p class="text-xs text-dark-400 mt-1">Active daemons</p>
                    </div>
                    <div class="w-12 h-12 rounded-xl bg-emerald-500/10 flex items-center justify-center">
                        <svg class="w-6 h-6 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M5.25 14.25h13.5m-13.5 0a3 3 0 01-3-3m3 3a3 3 0 100 6h13.5a3 3 0 100-6m-16.5-3a3 3 0 013-3h13.5a3 3 0 013 3m-19.5 0a4.5 4.5 0 01.9-2.7L5.737 5.1a3.375 3.375 0 012.7-1.35h7.126c1.062 0 2.062.5 2.7 1.35l2.587 3.45a4.5 4.5 0 01.9 2.7m0 0a3 3 0 01-3 3m0 3h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008zm-3 6h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008z" />
                        </svg>
                    </div>
                </div>
            </div>

            <!-- Users -->
            <div class="card p-6 animate-slide-up" style="animation-delay: 100ms;">
                <div class="flex items-center justify-between">
                    <div>
                        <p class="text-dark-400 text-sm font-medium">Users</p>
                        <p class="text-3xl font-bold text-white mt-1">{users.length || '—'}</p>
                        <p class="text-xs text-dark-400 mt-1">Registered accounts</p>
                    </div>
                    <div class="w-12 h-12 rounded-xl bg-amber-500/10 flex items-center justify-center">
                        <svg class="w-6 h-6 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z" />
                        </svg>
                    </div>
                </div>
            </div>

            <!-- System Resources -->
            <div class="card p-6 animate-slide-up" style="animation-delay: 150ms;">
                <div class="flex items-center justify-between">
                    <div>
                        <p class="text-dark-400 text-sm font-medium">Memory Usage</p>
                        <p class="text-3xl font-bold text-white mt-1">
                            {totalMemory > 0 ? Math.round((usedMemory / totalMemory) * 100) : 0}%
                        </p>
                        <p class="text-xs text-dark-400 mt-1">{usedMemory}MB / {totalMemory}MB</p>
                    </div>
                    <div class="w-12 h-12 rounded-xl bg-purple-500/10 flex items-center justify-center">
                        <svg class="w-6 h-6 text-purple-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M8.25 3v1.5M4.5 8.25H3m18 0h-1.5M4.5 12H3m18 0h-1.5m-15 3.75H3m18 0h-1.5M8.25 19.5V21M12 3v1.5m0 15V21m3.75-18v1.5m0 15V21m-9-1.5h10.5a2.25 2.25 0 002.25-2.25V6.75a2.25 2.25 0 00-2.25-2.25H6.75A2.25 2.25 0 004.5 6.75v10.5a2.25 2.25 0 002.25 2.25zm.75-12h9v9h-9v-9z" />
                        </svg>
                    </div>
                </div>
            </div>
        </div>

        <!-- Quick Links -->
        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-4">
            <a href="/admin/servers" class="card-hover p-6 group">
                <div class="flex items-center gap-4">
                    <div class="w-12 h-12 rounded-xl bg-gradient-to-br from-primary-500/20 to-primary-600/10 flex items-center justify-center group-hover:from-primary-500/30 group-hover:to-primary-600/20 transition-all duration-300">
                        <svg class="w-6 h-6 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                        </svg>
                    </div>
                    <div>
                        <h3 class="text-lg font-semibold text-white group-hover:text-primary-400 transition-colors duration-200">Create Server</h3>
                        <p class="text-sm text-dark-400">Deploy a new server with custom resources</p>
                    </div>
                </div>
            </a>

            <a href="/admin/users" class="card-hover p-6 group">
                <div class="flex items-center gap-4">
                    <div class="w-12 h-12 rounded-xl bg-gradient-to-br from-amber-500/20 to-amber-600/10 flex items-center justify-center group-hover:from-amber-500/30 group-hover:to-amber-600/20 transition-all duration-300">
                        <svg class="w-6 h-6 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M18 18.72a9.094 9.094 0 003.741-.479 3 3 0 00-4.682-2.72m.94 3.198l.001.031c0 .225-.012.447-.037.666A11.944 11.944 0 0112 21c-2.17 0-4.207-.576-5.963-1.584A6.062 6.062 0 016 18.719m12 0a5.971 5.971 0 00-.941-3.197m0 0A5.995 5.995 0 0012 12.75a5.995 5.995 0 00-5.058 2.772m0 0a3 3 0 00-4.681 2.72 8.986 8.986 0 003.74.477m.94-3.197a5.971 5.971 0 00-.94 3.197M15 6.75a3 3 0 11-6 0 3 3 0 016 0zm6 3a2.25 2.25 0 11-4.5 0 2.25 2.25 0 014.5 0zm-13.5 0a2.25 2.25 0 11-4.5 0 2.25 2.25 0 014.5 0z" />
                        </svg>
                    </div>
                    <div>
                        <h3 class="text-lg font-semibold text-white group-hover:text-amber-400 transition-colors duration-200">Manage Users</h3>
                        <p class="text-sm text-dark-400">View and manage user accounts</p>
                    </div>
                </div>
            </a>

            <a href="/admin/allocations" class="card-hover p-6 group">
                <div class="flex items-center gap-4">
                    <div class="w-12 h-12 rounded-xl bg-gradient-to-br from-emerald-500/20 to-emerald-600/10 flex items-center justify-center group-hover:from-emerald-500/30 group-hover:to-emerald-600/20 transition-all duration-300">
                        <svg class="w-6 h-6 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 21a9.004 9.004 0 008.716-6.747M12 21a9.004 9.004 0 01-8.716-6.747M12 21c2.485 0 4.5-4.03 4.5-9S14.485 3 12 3m0 18c-2.485 0-4.5-4.03-4.5-9S9.515 3 12 3m0 0a8.997 8.997 0 017.843 4.582M12 3a8.997 8.997 0 00-7.843 4.582m15.686 0A11.953 11.953 0 0112 10.5c-2.998 0-5.74-1.1-7.843-2.918m15.686 0A8.959 8.959 0 0121 12c0 .778-.099 1.533-.284 2.253m0 0A17.919 17.919 0 0112 16.5c-3.162 0-6.133-.815-8.716-2.247m0 0A9.015 9.015 0 013 12c0-1.605.42-3.113 1.157-4.418" />
                        </svg>
                    </div>
                    <div>
                        <h3 class="text-lg font-semibold text-white group-hover:text-emerald-400 transition-colors duration-200">Allocations</h3>
                        <p class="text-sm text-dark-400">Manage IP and port allocations</p>
                    </div>
                </div>
            </a>

            <a href="/admin/flakes" class="card-hover p-6 group">
                <div class="flex items-center gap-4">
                    <div class="w-12 h-12 rounded-xl bg-gradient-to-br from-purple-500/20 to-purple-600/10 flex items-center justify-center group-hover:from-purple-500/30 group-hover:to-purple-600/20 transition-all duration-300">
                        <svg class="w-6 h-6 text-purple-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a6.759 6.759 0 010 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 010-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28z" />
                            <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                        </svg>
                    </div>
                    <div>
                        <h3 class="text-lg font-semibold text-white group-hover:text-purple-400 transition-colors duration-200">Flakes</h3>
                        <p class="text-sm text-dark-400">Server templates and configurations</p>
                    </div>
                </div>
            </a>
        </div>

        <!-- Recent Servers Table -->
        <div>
            <div class="flex items-center justify-between mb-6">
                <h2 class="text-lg font-semibold text-white">All Servers</h2>
                <a href="/admin/servers" class="btn-primary btn-sm">
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                    </svg>
                    New Server
                </a>
            </div>

            <div class="table-container">
                <table class="table">
                    <thead>
                        <tr>
                            <th>Server</th>
                            <th>Image</th>
                            <th>Resources</th>
                            <th>Status</th>
                            <th class="text-right">Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each containers.slice(0, 10) as container}
                            <tr>
                                <td>
                                    <div class="flex items-center gap-3">
                                        <div class="w-9 h-9 rounded-lg bg-gradient-to-br from-primary-500/20 to-primary-600/10 flex items-center justify-center">
                                            <svg class="w-4 h-4 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                                <path stroke-linecap="round" stroke-linejoin="round" d="M21 7.5l-9-5.25L3 7.5m18 0l-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9" />
                                            </svg>
                                        </div>
                                        <div>
                                            <a href="/containers/{container.id}" class="font-medium text-white hover:text-primary-400 transition-colors duration-200">
                                                {container.name}
                                            </a>
                                            <p class="text-xs text-dark-500">{container.id.slice(0, 8)}</p>
                                        </div>
                                    </div>
                                </td>
                                <td>
                                    <code class="text-xs bg-dark-800 px-2 py-1 rounded text-dark-300">{container.image}</code>
                                </td>
                                <td>
                                    <div class="flex items-center gap-3 text-xs">
                                        <span class="text-dark-400">
                                            <span class="text-white font-medium">{container.memoryLimit || '—'}</span> MB
                                        </span>
                                        <span class="text-dark-400">
                                            <span class="text-white font-medium">{container.cpuLimit || '—'}</span> CPU
                                        </span>
                                    </div>
                                </td>
                                <td>
                                    <span class="{container.status.toLowerCase() === 'running' ? 'badge-success' : container.status.toLowerCase() === 'stopped' ? 'badge-danger' : 'badge-neutral'}">
                                        <span class="w-1.5 h-1.5 rounded-full {container.status.toLowerCase() === 'running' ? 'bg-emerald-400 animate-pulse' : 'bg-current'}"></span>
                                        {container.status}
                                    </span>
                                </td>
                                <td class="text-right">
                                    <a href="/containers/{container.id}" class="btn-ghost btn-sm">
                                        Manage
                                    </a>
                                </td>
                            </tr>
                        {:else}
                            <tr>
                                <td colspan="5" class="text-center py-8 text-dark-400">
                                    No servers found. Create your first server to get started.
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        </div>
    {/if}
</div>
