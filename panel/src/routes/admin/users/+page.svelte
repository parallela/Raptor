<script lang="ts">
    import { onMount } from 'svelte';
    import { api } from '$lib/api';
    import { user } from '$lib/stores';
    import { goto } from '$app/navigation';
    import toast from 'svelte-french-toast';
    import { _ } from '$lib/i18n';
    import type { User } from '$lib/types';

    let users: User[] = [];
    let loading = true;
    let error = '';

    let showInviteModal = false;
    let inviteEmail = '';
    let inviteRoleId = '';
    let inviting = false;
    let roles: { id: string; name: string }[] = [];

    onMount(async () => {
        if (!$user) {
            goto('/login');
            return;
        }
        await loadData();
        await loadRoles();
    });

    async function loadData() {
        loading = true;
        error = '';
        try {
            users = await api.listUsers();
        } catch (e: any) {
            error = e.message || 'Failed to load users. You may not have admin permissions.';
            users = [];
        } finally {
            loading = false;
        }
    }

    async function loadRoles() {
        try {
            roles = await api.listRoles();
        } catch (e) {
            console.error('Failed to load roles:', e);
        }
    }

    async function deleteUser(id: string, username: string) {
        if (!confirm(`Are you sure you want to delete user "${username}"? This action cannot be undone.`)) return;
        try {
            await api.deleteUser(id);
            await loadData();
            toast.success(`User "${username}" deleted successfully`);
        } catch (e: any) {
            toast.error(e.message || 'Failed to delete user');
        }
    }

    async function inviteUser() {
        if (!inviteEmail.trim()) {
            toast.error('Please enter an email address');
            return;
        }
        inviting = true;
        try {
            const result = await api.inviteUser(inviteEmail.trim(), inviteRoleId || undefined);
            toast.success('Invitation sent successfully');
            showInviteModal = false;
            inviteEmail = '';
            inviteRoleId = '';
            if (result.token) {
                const inviteLink = `${window.location.origin}/invite?token=${result.token}`;
                await navigator.clipboard.writeText(inviteLink);
                toast.success('Invite link copied to clipboard', { duration: 5000 });
            }
        } catch (e: any) {
            toast.error(e.message || 'Failed to send invitation');
        } finally {
            inviting = false;
        }
    }
</script>

<div class="space-y-4 md:space-y-6">
    <!-- Header -->
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div class="flex items-center gap-3 md:gap-4">
            <a href="/admin" class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-800 transition-colors duration-200">
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" />
                </svg>
            </a>
            <div>
                <h1 class="text-xl md:text-2xl font-bold text-white">{$_('adminUsers.title')}</h1>
                <p class="text-sm text-dark-400">{$_('adminUsers.subtitle')}</p>
            </div>
        </div>
        <button on:click={() => showInviteModal = true} class="btn-primary w-full sm:w-auto">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
            {$_('adminUsers.inviteUser')}
        </button>
    </div>

    {#if error}
        <div class="flex items-center gap-3 p-4 rounded-lg bg-red-500/10 border border-red-500/20">
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
                <p class="text-dark-400">{$_('common.loading')}</p>
            </div>
        </div>
    {:else if users.length > 0}
        <!-- Mobile Card View -->
        <div class="md:hidden space-y-3">
            {#each users as u, i}
                <div class="card p-4 animate-slide-up" style="animation-delay: {i * 30}ms;">
                    <div class="flex items-center justify-between">
                        <div class="flex items-center gap-3">
                            <div class="w-10 h-10 rounded-full bg-gradient-to-br from-primary-400 to-primary-600 flex items-center justify-center text-white font-semibold">
                                {u.username.charAt(0).toUpperCase()}
                            </div>
                            <div>
                                <p class="font-medium text-white">{u.username}</p>
                                <span class="{u.roleName === 'admin' ? 'badge-warning' : 'badge-neutral'} text-xs">
                                    {u.roleName || 'User'}
                                </span>
                            </div>
                        </div>
                        <button
                            on:click={() => deleteUser(u.id, u.username)}
                            class="btn-ghost btn-sm text-red-400 hover:text-red-300 hover:bg-red-500/10"
                            disabled={u.id === $user?.id}
                        >
                            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0" />
                            </svg>
                        </button>
                    </div>
                    <div class="mt-2 pt-2 border-t border-dark-700/50">
                        <code class="text-xs bg-dark-800 px-2 py-1 rounded text-dark-400">{u.id.slice(0, 20)}...</code>
                    </div>
                </div>
            {/each}
        </div>

        <!-- Desktop Table View -->
        <div class="hidden md:block table-container">
            <table class="table">
                <thead>
                    <tr>
                        <th>User</th>
                        <th>Role</th>
                        <th>ID</th>
                        <th class="text-right">Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {#each users as u, i}
                        <tr class="animate-slide-up" style="animation-delay: {i * 30}ms;">
                            <td>
                                <div class="flex items-center gap-3">
                                    <div class="w-9 h-9 rounded-full bg-gradient-to-br from-primary-400 to-primary-600 flex items-center justify-center text-white font-semibold text-sm">
                                        {u.username.charAt(0).toUpperCase()}
                                    </div>
                                    <div>
                                        <p class="font-medium text-white">{u.username}</p>
                                        {#if u.roleName === 'admin'}
                                            <span class="badge-info text-xs">Admin</span>
                                        {/if}
                                    </div>
                                </div>
                            </td>
                            <td>
                                <span class="{u.roleName === 'admin' ? 'badge-warning' : 'badge-neutral'}">
                                    {u.roleName || 'User'}
                                </span>
                            </td>
                            <td>
                                <code class="text-xs bg-dark-800 px-2 py-1 rounded text-dark-400">{u.id.slice(0, 12)}...</code>
                            </td>
                            <td class="text-right">
                                <button
                                    on:click={() => deleteUser(u.id, u.username)}
                                    class="btn-ghost btn-sm text-red-400 hover:text-red-300 hover:bg-red-500/10"
                                    disabled={u.id === $user?.id}
                                >
                                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0" />
                                    </svg>
                                    Delete
                                </button>
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {:else}
        <div class="card p-12 text-center">
            <div class="w-16 h-16 rounded-2xl bg-dark-800 flex items-center justify-center mx-auto mb-4">
                <svg class="w-8 h-8 text-dark-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z" />
                </svg>
            </div>
            <h3 class="text-lg font-semibold text-white mb-2">No users found</h3>
            <p class="text-dark-400">User data is not available or you don't have permission to view it.</p>
        </div>
    {/if}
</div>

<!-- Invite User Modal -->
{#if showInviteModal}
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
        <div class="absolute inset-0 bg-dark-950/80 backdrop-blur-sm" on:click={() => showInviteModal = false} role="button" tabindex="-1" on:keydown={(e) => e.key === 'Escape' && (showInviteModal = false)}></div>
        <div class="relative card p-6 max-w-md w-full animate-slide-up">
            <div class="flex items-center justify-between mb-6">
                <h2 class="text-xl font-semibold text-white">Invite User</h2>
                <button on:click={() => showInviteModal = false} class="text-dark-400 hover:text-white">
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
            </div>

            <form on:submit|preventDefault={inviteUser} class="space-y-4">
                <div class="input-group">
                    <label for="invite-email" class="input-label">Email Address</label>
                    <input
                        type="email"
                        id="invite-email"
                        bind:value={inviteEmail}
                        class="input"
                        placeholder="user@example.com"
                        required
                    />
                    <p class="text-xs text-dark-500 mt-1">An invitation link will be sent to this email address.</p>
                </div>

                <div class="input-group">
                    <label for="invite-role" class="input-label">
                        Role
                        <span class="text-dark-500 font-normal">(optional)</span>
                    </label>
                    <select id="invite-role" bind:value={inviteRoleId} class="input">
                        <option value="">Default (User)</option>
                        {#each roles as role}
                            <option value={role.id}>{role.name}</option>
                        {/each}
                    </select>
                </div>

                <div class="flex gap-3 pt-2">
                    <button type="submit" class="btn-primary flex-1" disabled={inviting}>
                        {#if inviting}
                            <span class="spinner w-4 h-4 mr-2"></span>
                        {/if}
                        Send Invitation
                    </button>
                    <button type="button" on:click={() => showInviteModal = false} class="btn-secondary">
                        Cancel
                    </button>
                </div>
            </form>
        </div>
    </div>
{/if}

