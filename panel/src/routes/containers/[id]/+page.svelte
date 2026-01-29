<script lang="ts">
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import { onMount } from 'svelte';

    $: containerId = $page.params.id;

    onMount(() => {
        // Redirect to console tab by default
        goto(`/containers/${containerId}/console`, { replaceState: true });
    });
</script>

<div class="flex items-center justify-center h-full">
    <div class="spinner w-6 h-6"></div>
</div>
