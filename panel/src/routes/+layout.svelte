<script lang="ts">
    import '../app.css';
    import { Toaster } from 'svelte-french-toast';
    import { user, token, isAdmin, isManager, canViewDaemons } from '$lib/stores';
    import { goto } from '$app/navigation';
    import { page } from '$app/stores';
    import { onMount } from 'svelte';
    import { _ } from '$lib/i18n';
    import { LocaleSelector } from '$lib/components';

    let sidebarOpen = false;
    let isMobile = false;

    function logout() {
        $user = null;
        $token = null;
        goto('/login');
    }

    function closeSidebar() {
        if (isMobile) sidebarOpen = false;
    }

    function checkMobile() {
        isMobile = window.innerWidth < 768;
        if (!isMobile) sidebarOpen = false;
    }

    onMount(() => {
        checkMobile();
        window.addEventListener('resize', checkMobile);
        return () => window.removeEventListener('resize', checkMobile);
    });

    $: currentPath = $page.url.pathname;

    // Close sidebar on navigation (mobile)
    $: if (currentPath && isMobile) {
        sidebarOpen = false;
    }

    $: navItems = [
        { href: '/', label: $_('nav.dashboard'), icon: 'dashboard', show: true },
        { href: '/containers', label: $_('nav.containers'), icon: 'containers', show: true },
        { href: '/databases', label: $_('nav.database'), icon: 'database', show: true },
        { href: '/daemons', label: $_('nav.daemons'), icon: 'daemons', show: $canViewDaemons },
        { href: '/admin', label: $_('nav.admin'), icon: 'admin', show: $isAdmin || $isManager },
    ].filter(item => item.show);

    $: roleName = $user?.roleName
        ? $user.roleName.charAt(0).toUpperCase() + $user.roleName.slice(1)
        : 'User';
</script>

<Toaster
    position="bottom-right"
    toastOptions={{
        duration: 4000,
        style: 'background: #1e293b; color: #e2e8f0; border: 1px solid #334155;'
    }}
/>

<div class="min-h-screen bg-dark-950">
    <!-- Background gradient effects -->
    <div class="fixed inset-0 overflow-hidden pointer-events-none">
        <div class="absolute -top-40 -right-40 w-80 h-80 bg-primary-500/10 rounded-full blur-3xl"></div>
        <div class="absolute top-1/2 -left-40 w-80 h-80 bg-primary-600/5 rounded-full blur-3xl"></div>
    </div>

    {#if $user}
        <div class="relative flex">
            <!-- Mobile Header -->
            <header class="md:hidden fixed top-0 left-0 right-0 h-14 bg-dark-900/95 backdrop-blur-xl border-b border-dark-700/50 z-50 flex items-center justify-between px-4">
                <button
                    on:click={() => sidebarOpen = !sidebarOpen}
                    class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-800 transition-colors"
                >
                    <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                        {#if sidebarOpen}
                            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                        {:else}
                            <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 6.75h16.5M3.75 12h16.5m-16.5 5.25h16.5" />
                        {/if}
                    </svg>
                </button>
                <a href="/" class="flex items-center">
                    <img src="/logo.webp" alt="Raptor" class="h-8 object-contain" />
                </a>
                <div class="flex items-center gap-2">
                    <LocaleSelector />
                    <div class="w-10 h-10 rounded-full bg-gradient-to-br from-primary-400 to-primary-600 flex items-center justify-center text-white font-semibold text-sm">
                        {$user.username.charAt(0).toUpperCase()}
                    </div>
                </div>
            </header>

            <!-- Mobile Overlay -->
            {#if sidebarOpen && isMobile}
                <div
                    class="fixed inset-0 bg-dark-950/80 backdrop-blur-sm z-40 md:hidden"
                    on:click={closeSidebar}
                    on:keydown={(e) => e.key === 'Escape' && closeSidebar()}
                    role="button"
                    tabindex="-1"
                ></div>
            {/if}

            <!-- Sidebar -->
            <aside class="fixed left-0 top-0 h-screen w-64 bg-dark-900/95 backdrop-blur-xl border-r border-dark-700/50 flex flex-col z-50 transition-transform duration-300 ease-in-out
                {isMobile ? (sidebarOpen ? 'translate-x-0' : '-translate-x-full') : 'translate-x-0'}">
                <!-- Logo -->
                <div class="h-28 flex items-center justify-center px-3 border-b border-dark-700/50 bg-gray-400">
                    <a href="/" class="flex items-center justify-center w-full" on:click={closeSidebar}>
                        <img src="/logo.webp" alt="Raptor" class="w-full object-contain brightness-110 contrast-110" />
                    </a>
                </div>

                <!-- Navigation -->
                <nav class="flex-1 px-3 py-6 space-y-1 overflow-y-auto">
                    {#each navItems as item}
                        <a
                            href={item.href}
                            on:click={closeSidebar}
                            class="flex items-center gap-3 px-4 py-3 rounded-lg text-sm font-medium transition-all duration-200 group
                                {currentPath === item.href || (item.href !== '/' && currentPath.startsWith(item.href))
                                    ? 'bg-primary-500/10 text-primary-400 border border-primary-500/20'
                                    : 'text-dark-400 hover:text-white hover:bg-dark-800/50'}"
                        >
                            {#if item.icon === 'dashboard'}
                                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 6A2.25 2.25 0 016 3.75h2.25A2.25 2.25 0 0110.5 6v2.25a2.25 2.25 0 01-2.25 2.25H6a2.25 2.25 0 01-2.25-2.25V6zM3.75 15.75A2.25 2.25 0 016 13.5h2.25a2.25 2.25 0 012.25 2.25V18a2.25 2.25 0 01-2.25 2.25H6A2.25 2.25 0 013.75 18v-2.25zM13.5 6a2.25 2.25 0 012.25-2.25H18A2.25 2.25 0 0120.25 6v2.25A2.25 2.25 0 0118 10.5h-2.25a2.25 2.25 0 01-2.25-2.25V6zM13.5 15.75a2.25 2.25 0 012.25-2.25H18a2.25 2.25 0 012.25 2.25V18A2.25 2.25 0 0118 20.25h-2.25A2.25 2.25 0 0113.5 18v-2.25z" />
                                </svg>
                            {:else if item.icon === 'containers'}
                                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M21 7.5l-9-5.25L3 7.5m18 0l-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9" />
                                </svg>
                            {:else if item.icon === 'database'}
                                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375m16.5 0v3.75m-16.5-3.75v3.75m16.5 0v3.75C20.25 16.153 16.556 18 12 18s-8.25-1.847-8.25-4.125v-3.75m16.5 0c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125" />
                                </svg>
                            {:else if item.icon === 'daemons'}
                                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M5.25 14.25h13.5m-13.5 0a3 3 0 01-3-3m3 3a3 3 0 100 6h13.5a3 3 0 100-6m-16.5-3a3 3 0 013-3h13.5a3 3 0 013 3m-19.5 0a4.5 4.5 0 01.9-2.7L5.737 5.1a3.375 3.375 0 012.7-1.35h7.126c1.062 0 2.062.5 2.7 1.35l2.587 3.45a4.5 4.5 0 01.9 2.7m0 0a3 3 0 01-3 3m0 3h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008zm-3 6h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008z" />
                                </svg>
                            {:else if item.icon === 'admin'}
                                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z" />
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                                </svg>
                            {/if}
                            {item.label}
                        </a>
                    {/each}
                </nav>

                <!-- User section -->
                <div class="p-4 border-t border-dark-700/50 space-y-3">
                    <div class="flex items-center justify-center">
                        <LocaleSelector />
                    </div>
                    <div class="flex items-center justify-between p-3 rounded-lg bg-dark-800/50">
                        <div class="flex items-center gap-3">
                            <div class="w-9 h-9 rounded-full bg-gradient-to-br from-primary-400 to-primary-600 flex items-center justify-center text-white font-semibold text-sm">
                                {$user.username.charAt(0).toUpperCase()}
                            </div>
                            <div class="flex flex-col">
                                <span class="text-sm font-medium text-white">{$user.username}</span>
                                <span class="text-xs text-dark-400">{roleName}</span>
                            </div>
                        </div>
                        <button
                            on:click={logout}
                            class="p-2 rounded-lg text-dark-400 hover:text-white hover:bg-dark-700/50 transition-colors duration-200"
                            title={$_('nav.logout')}
                        >
                            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 9V5.25A2.25 2.25 0 0013.5 3h-6a2.25 2.25 0 00-2.25 2.25v13.5A2.25 2.25 0 007.5 21h6a2.25 2.25 0 002.25-2.25V15m3 0l3-3m0 0l-3-3m3 3H9" />
                            </svg>
                        </button>
                    </div>
                </div>
            </aside>

            <!-- Main content -->
            <main class="flex-1 md:ml-64 min-h-screen flex flex-col pt-14 md:pt-0">
                <div class="flex-1 p-4 md:p-8 animate-fade-in">
                    <slot />
                </div>
                <footer class="p-4 text-center text-xs text-dark-500 border-t border-dark-800">
                    Raptor v1.0.0 - Made by <a href="https://github.com/parallela" target="_blank" rel="noopener noreferrer" class="text-dark-400 hover:text-white transition-colors">parallela</a>
                </footer>
            </main>
        </div>
    {:else}
        <main class="min-h-screen flex flex-col">
            <div class="flex-1">
                <slot />
            </div>
            <footer class="p-4 text-center text-xs text-dark-500 border-t border-dark-800">
                Raptor v1.0.0 - Made by <a href="https://github.com/parallela" target="_blank" rel="noopener noreferrer" class="text-dark-400 hover:text-white transition-colors">parallela</a>
            </footer>
        </main>
    {/if}
</div>
