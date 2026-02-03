<script lang="ts">
    import { page } from '$app/stores';
    import { getContext, onMount } from 'svelte';
    import type { Writable } from 'svelte/store';
    import { api } from '$lib/api';
    import toast from 'svelte-french-toast';
    import type { Container, Daemon } from '$lib/types';

    const containerStore = getContext<Writable<Container | null>>('container');

    let sftpPassword = '';
    let sftpPasswordConfirm = '';
    let settingSftpPassword = false;
    let copiedField: string | null = null;
    let daemon: Daemon | null = null;

    $: containerId = $page.params.id as string;
    $: container = $containerStore;
    $: hasSftpPassword = !!container?.sftpPass;

    onMount(async () => {
        if (container?.daemonId) {
            try {
                daemon = await api.getDaemon(container.daemonId);
            } catch (e) {
                console.error('Failed to load daemon info:', e);
            }
        }
    });

    $: if (container?.daemonId && !daemon) {
        api.getDaemon(container.daemonId).then(d => daemon = d).catch(() => {});
    }

    async function setSftpPassword() {
        if (sftpPassword !== sftpPasswordConfirm) {
            toast.error('Passwords do not match');
            return;
        }
        if (sftpPassword.length < 8) {
            toast.error('Password must be at least 8 characters');
            return;
        }
        settingSftpPassword = true;
        try {
            await api.setSftpPassword(containerId, sftpPassword);
            toast.success('FTP password set');
            sftpPassword = '';
            sftpPasswordConfirm = '';
            const updated = await api.getContainer(containerId);
            containerStore.set(updated);
        } catch (e: any) {
            toast.error(e.message || 'Failed to set FTP password');
        } finally {
            settingSftpPassword = false;
        }
    }

    function copyToClipboard(text: string, field: string) {
        navigator.clipboard.writeText(text);
        copiedField = field;
        toast.success('Copied to clipboard');
        setTimeout(() => copiedField = null, 2000);
    }

    $: ftpHost = daemon?.host || container?.allocationIp || 'localhost';
    $: ftpPort = 2121;
    $: ftpUsername = containerId.slice(0, 8);
</script>

<svelte:head>
    <title>{container?.name || 'Container'} - FTP - Raptor</title>
</svelte:head>

<div class="h-full overflow-y-auto p-6">
    <div class="max-w-2xl mx-auto space-y-6">
        <div>
            <h2 class="text-lg font-semibold text-white mb-1">FTP Connection</h2>
            <p class="text-dark-400 text-sm">Use these credentials to connect via FTP client (FileZilla, WinSCP, etc.)</p>
        </div>

        <div class="bg-dark-900 rounded-lg border border-dark-700 overflow-hidden">
            <!-- Connection Details -->
            <div class="p-4 space-y-4">
                <div class="grid gap-4 sm:grid-cols-2">
                    <div>
                        <span class="block text-xs text-dark-400 uppercase mb-1">Host</span>
                        <div class="flex items-center gap-2">
                            <code class="flex-1 bg-dark-800 text-white px-3 py-2 rounded text-sm font-mono">{ftpHost}</code>
                            <button
                                on:click={() => copyToClipboard(ftpHost, 'host')}
                                class="p-2 text-dark-400 hover:text-white hover:bg-dark-700 rounded transition-colors"
                                title="Copy">
                                {#if copiedField === 'host'}
                                    <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                                    </svg>
                                {:else}
                                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                                    </svg>
                                {/if}
                            </button>
                        </div>
                    </div>
                    <div>
                        <span class="block text-xs text-dark-400 uppercase mb-1">Port</span>
                        <div class="flex items-center gap-2">
                            <code class="flex-1 bg-dark-800 text-white px-3 py-2 rounded text-sm font-mono">{ftpPort}</code>
                            <button
                                on:click={() => copyToClipboard(ftpPort.toString(), 'port')}
                                class="p-2 text-dark-400 hover:text-white hover:bg-dark-700 rounded transition-colors"
                                title="Copy">
                                {#if copiedField === 'port'}
                                    <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                                    </svg>
                                {:else}
                                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                                    </svg>
                                {/if}
                            </button>
                        </div>
                    </div>
                </div>

                <div>
                    <span class="block text-xs text-dark-400 uppercase mb-1">Username</span>
                    <div class="flex items-center gap-2">
                        <code class="flex-1 bg-dark-800 text-white px-3 py-2 rounded text-sm font-mono">{ftpUsername}</code>
                        <button
                            on:click={() => copyToClipboard(ftpUsername, 'username')}
                            class="p-2 text-dark-400 hover:text-white hover:bg-dark-700 rounded transition-colors"
                            title="Copy">
                            {#if copiedField === 'username'}
                                <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                                </svg>
                            {:else}
                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                                </svg>
                            {/if}
                        </button>
                    </div>
                </div>
            </div>

            <!-- Password Section -->
            <div class="border-t border-dark-700 p-4">
                <div class="flex items-center gap-2 mb-3">
                    <h3 class="text-sm font-medium text-white">Password</h3>
                    {#if hasSftpPassword}
                        <span class="px-2 py-0.5 text-xs bg-emerald-500/10 text-emerald-400 rounded">Set</span>
                    {:else}
                        <span class="px-2 py-0.5 text-xs bg-yellow-500/10 text-yellow-400 rounded">Not set</span>
                    {/if}
                </div>

                <form on:submit|preventDefault={setSftpPassword} class="space-y-3">
                    <div class="grid gap-3 sm:grid-cols-2">
                        <input
                            type="password"
                            bind:value={sftpPassword}
                            placeholder="New password"
                            class="input w-full text-sm"
                            minlength="8"
                        />
                        <input
                            type="password"
                            bind:value={sftpPasswordConfirm}
                            placeholder="Confirm password"
                            class="input w-full text-sm"
                        />
                    </div>
                    <button type="submit" disabled={settingSftpPassword || !sftpPassword || !sftpPasswordConfirm} class="btn-primary text-sm">
                        {#if settingSftpPassword}
                            <span class="spinner w-4 h-4"></span>
                        {:else}
                            {hasSftpPassword ? 'Update Password' : 'Set Password'}
                        {/if}
                    </button>
                </form>
            </div>
        </div>

        <!-- Quick Connect URLs -->
        <div class="bg-dark-900 rounded-lg border border-dark-700 p-4">
            <h3 class="text-sm font-medium text-white mb-3">Quick Connect</h3>
            <div class="space-y-2">
                <div>
                    <span class="block text-xs text-dark-400 mb-1">FTP URL</span>
                    <div class="flex items-center gap-2">
                        <code class="flex-1 bg-dark-800 text-primary-400 px-3 py-2 rounded text-sm font-mono truncate">ftp:
                        <button
                            on:click={() => copyToClipboard(`ftp://${ftpUsername}@${ftpHost}:${ftpPort}`, 'url')}
                            class="p-2 text-dark-400 hover:text-white hover:bg-dark-700 rounded transition-colors"
                            title="Copy">
                            {#if copiedField === 'url'}
                                <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                                </svg>
                            {:else}
                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                                </svg>
                            {/if}
                        </button>
                    </div>
                </div>
            </div>
        </div>

        <!-- Tips -->
        <div class="bg-dark-900/50 rounded-lg border border-dark-700/50 p-4">
            <h3 class="text-sm font-medium text-white mb-2">Tips</h3>
            <ul class="text-sm text-dark-400 space-y-1 list-disc list-inside">
                <li>Use an FTP client like <span class="text-primary-400">FileZilla</span> or <span class="text-primary-400">WinSCP</span> for best experience</li>
                <li>Make sure to use <span class="text-white">Plain FTP</span> (not SFTP or FTPS) unless your server supports it</li>
                <li>Passive mode is recommended for most connections</li>
                <li>Your password is securely hashed and cannot be recovered</li>
            </ul>
        </div>
    </div>
</div>
