import { get } from 'svelte/store';
import { token } from './stores';
import type { Container, Daemon, Allocation, ContainerAllocation, User, ResourceLimits, ContainerPort } from './types';

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000';

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
    const t = get(token);
    const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        ...(options.headers as Record<string, string>),
    };
    
    if (t) {
        headers['Authorization'] = `Bearer ${t}`;
    }

    const res = await fetch(`${API_URL}${path}`, {
        ...options,
        headers,
    });

    if (!res.ok) {
        const error = await res.json().catch(() => ({ error: res.statusText }));
        throw new Error(error.error || res.statusText);
    }

    return res.json();
}

async function requestText(path: string): Promise<string> {
    const t = get(token);
    const headers: Record<string, string> = {};

    if (t) {
        headers['Authorization'] = `Bearer ${t}`;
    }

    const res = await fetch(`${API_URL}${path}`, { headers });

    if (!res.ok) {
        const error = await res.json().catch(() => ({ error: res.statusText }));
        throw new Error(error.error || res.statusText);
    }

    return res.text();
}

export interface CreateContainerData {
    daemonId: string;
    name: string;
    // Either flakeId or image is required
    flakeId?: string;
    image?: string;
    startupScript?: string;
    allocationId?: string;
    // Resource limits
    memoryLimit?: number;      // Docker container memory limit
    serverMemory?: number;     // Server/JVM heap memory (-Xmx)
    cpuLimit?: number;
    diskLimit?: number;
    swapLimit?: number;
    ioWeight?: number;
    // Optional user assignment (admin only)
    userId?: string;
    // Flake variables (envVariable -> value)
    variables?: Record<string, string>;
}

export const api = {
    // Auth
    login: (username: string, password: string) =>
        request<{ token: string; user: User }>('/auth/login', {
            method: 'POST',
            body: JSON.stringify({ username, password }),
        }),
    
    register: (username: string, email: string, password: string) =>
        request<User>('/auth/register', {
            method: 'POST',
            body: JSON.stringify({ username, email, password }),
        }),

    forgotPassword: (email: string) =>
        request<{ message: string }>('/auth/forgot-password', {
            method: 'POST',
            body: JSON.stringify({ email }),
        }),

    resetPassword: (token: string, password: string) =>
        request<{ message: string }>('/auth/reset-password', {
            method: 'POST',
            body: JSON.stringify({ token, password }),
        }),

    // Containers
    listContainers: () => request<Container[]>('/containers'),
    getContainer: (id: string) => request<Container>(`/containers/${id}`),
    getContainerPorts: (id: string) => request<ContainerPort[]>(`/containers/${id}/ports`),
    createContainer: (data: CreateContainerData) =>
        request<Container>('/containers', { method: 'POST', body: JSON.stringify(data) }),
    updateContainer: (id: string, data: Partial<CreateContainerData>) =>
        request<Container>(`/containers/${id}`, { method: 'PATCH', body: JSON.stringify(data) }),
    deleteContainer: (id: string) => request<void>(`/containers/${id}`, { method: 'DELETE' }),
    assignAllocation: (id: string, allocationId: string) =>
        request<{ message: string; allocationIp: string; allocationPort: number }>(`/containers/${id}/allocation`, {
            method: 'POST',
            body: JSON.stringify({ allocationId })
        }),
    getContainerAllocations: (id: string) => request<ContainerAllocation[]>(`/containers/${id}/allocations`),
    getAvailableAllocations: (daemonId: string) => request<Allocation[]>(`/containers/${daemonId}/allocations/available`),
    addContainerAllocation: (containerId: string, allocationId: string) =>
        request<{ message: string; allocationIp: string; allocationPort: number }>(`/containers/${containerId}/allocations`, {
            method: 'POST',
            body: JSON.stringify({ allocationId })
        }),
    removeContainerAllocation: (containerId: string, allocationId: string) =>
        request<{ message: string }>(`/containers/${containerId}/allocations/${allocationId}`, { method: 'DELETE' }),
    setContainerPrimaryAllocation: (containerId: string, allocationId: string) =>
        request<{ message: string }>(`/containers/${containerId}/allocations/${allocationId}/primary`, { method: 'POST' }),
    addAllocation: (id: string, allocationId: string) =>
        request<{ message: string; allocationIp: string; allocationPort: number }>(`/containers/${id}/allocations`, {
            method: 'POST',
            body: JSON.stringify({ allocationId })
        }),
    removeAllocation: (id: string, allocationId: string) =>
        request<{ message: string }>(`/containers/${id}/allocations/${allocationId}`, { method: 'DELETE' }),
    startContainer: (id: string) => request<Container>(`/containers/${id}/start`, { method: 'POST' }),
    stopContainer: (id: string) => request<Container>(`/containers/${id}/stop`, { method: 'POST' }),
    restartContainer: (id: string) => request<Container>(`/containers/${id}/restart`, { method: 'POST' }),
    killContainer: (id: string) => request<Container>(`/containers/${id}/kill`, { method: 'POST' }),
    sendCommand: (id: string, command: string) =>
        request<{ success: boolean }>(`/containers/${id}/command`, {
            method: 'POST',
            body: JSON.stringify({ command })
        }),
    gracefulStop: (id: string, timeoutSecs: number = 30) =>
        request<{ success: boolean }>(`/containers/${id}/graceful-stop`, {
            method: 'POST',
            body: JSON.stringify({ timeoutSecs })
        }),
    getContainerStats: (id: string) => request<{ cpuPercent: number; memoryUsage: number; memoryLimit: number; memoryPercent: number; networkRx: number; networkTx: number; blockRead: number; blockWrite: number }>(`/containers/${id}/stats`),
    setSftpPassword: (id: string, password: string) =>
        request<{ message: string }>(`/containers/${id}/sftp-password`, {
            method: 'POST',
            body: JSON.stringify({ password })
        }),

    // Daemons
    listDaemons: () => request<Daemon[]>('/daemons'),
    getDaemon: (id: string) => request<Daemon>(`/daemons/${id}`),
    getDaemonStatus: (id: string) => request<{ id: string; status: string; system?: { totalMemory: number; availableMemory: number; cpuCores: number; cpuUsage: number; totalDisk: number; availableDisk: number; hostname: string } }>(`/daemons/${id}/status`),
    createDaemon: (data: { name: string; host: string; port: number; location?: string; secure?: boolean; totalMemory?: number; totalCpu?: number; totalDisk?: number }) =>
        request<Daemon>('/admin/daemons', { method: 'POST', body: JSON.stringify(data) }),
    updateDaemon: (id: string, data: Partial<Daemon>) =>
        request<Daemon>(`/admin/daemons/${id}`, { method: 'PATCH', body: JSON.stringify(data) }),
    deleteDaemon: (id: string) => request<void>(`/admin/daemons/${id}`, { method: 'DELETE' }),
    pingDaemon: (data: { host: string; port: number; apiKey: string; secure?: boolean }) =>
        request<{ online: boolean; latencyMs?: number; version?: string; system?: any; error?: string }>('/admin/daemons/ping', { method: 'POST', body: JSON.stringify(data) }),

    // Allocations
    listAllocations: () => request<Allocation[]>('/allocations'),
    listAllAllocations: () => request<Allocation[]>('/allocations/all'),
    createAllocation: (data: { daemonId: string; ip: string; port: number }) =>
        request<Allocation>('/allocations', { method: 'POST', body: JSON.stringify(data) }),
    deleteAllocation: (id: string) => request<void>(`/allocations/${id}`, { method: 'DELETE' }),

    // Users
    getMe: () => request<User>('/users/me'),
    listUsers: async (): Promise<User[]> => {
        const response = await request<{ data: User[]; total: number; page: number; perPage: number; totalPages: number }>('/users');
        return response.data;
    },
    getUser: (id: string) => request<User>(`/users/${id}`),
    updateUser: (id: string, data: { username?: string; roleId?: string }) =>
        request<User>(`/users/${id}`, { method: 'PATCH', body: JSON.stringify(data) }),
    deleteUser: (id: string) => request<void>(`/admin/users/${id}`, { method: 'DELETE' }),
    searchUsers: (query: string, limit: number = 10) =>
        request<User[]>(`/admin/users/search?q=${encodeURIComponent(query)}&limit=${limit}`),
    inviteUser: (email: string, roleId?: string) =>
        request<{ message: string; token: string }>('/admin/users/invite', {
            method: 'POST',
            body: JSON.stringify({ email, roleId })
        }),
    acceptInvite: (token: string, username: string, password: string) =>
        request<User>('/auth/accept-invite', {
            method: 'POST',
            body: JSON.stringify({ token, username, password })
        }),

    // Roles
    listRoles: () => request<{ id: string; name: string; permissions: Record<string, boolean> }[]>('/roles'),
    getRole: (id: string) => request<{ id: string; name: string; permissions: Record<string, boolean> }>(`/roles/${id}`),
    createRole: (data: { name: string; permissions: Record<string, boolean> }) =>
        request<{ id: string; name: string; permissions: Record<string, boolean> }>('/admin/roles', { method: 'POST', body: JSON.stringify(data) }),
    updateRole: (id: string, data: { name: string; permissions: Record<string, boolean> }) =>
        request<{ id: string; name: string; permissions: Record<string, boolean> }>(`/admin/roles/${id}`, { method: 'PATCH', body: JSON.stringify(data) }),
    deleteRole: (id: string) => request<void>(`/admin/roles/${id}`, { method: 'DELETE' }),

    // Files
    listFiles: (containerId: string, path: string = '/') =>
        request<{ name: string; isDir: boolean; size: number; modified?: string }[]>(`/containers/${containerId}/files?path=${encodeURIComponent(path)}`),
    readFile: (containerId: string, path: string) =>
        requestText(`/containers/${containerId}/files/read?path=${encodeURIComponent(path)}`),
    writeFile: (containerId: string, path: string, content: string) =>
        request<{ message: string }>(`/containers/${containerId}/files/write`, { method: 'POST', body: JSON.stringify({ path, content }) }),
    createFolder: (containerId: string, path: string) =>
        request<{ message: string }>(`/containers/${containerId}/files/folder`, { method: 'POST', body: JSON.stringify({ path }) }),
    deleteFile: (containerId: string, path: string) =>
        request<{ message: string }>(`/containers/${containerId}/files/delete?path=${encodeURIComponent(path)}`, { method: 'DELETE' }),
    uploadFile: async (containerId: string, path: string, file: File) => {
        const t = get(token);
        const formData = new FormData();
        formData.append('file', file);
        formData.append('path', path);

        const response = await fetch(`${import.meta.env.VITE_API_URL || 'http://localhost:3000'}/containers/${containerId}/files/upload`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${t}`
            },
            body: formData
        });

        if (!response.ok) {
            const data = await response.json().catch(() => ({}));
            throw new Error(data.error || 'Upload failed');
        }
        return response.json();
    },

    // Aliases for container file operations
    listContainerFiles: (containerId: string, path: string = '/') =>
        request<{ name: string; isDir: boolean; size: number; modified?: string }[]>(`/containers/${containerId}/files?path=${encodeURIComponent(path)}`),
    getContainerFile: (containerId: string, path: string) =>
        requestText(`/containers/${containerId}/files/read?path=${encodeURIComponent(path)}`),
    saveContainerFile: (containerId: string, path: string, content: string) =>
        request<{ message: string }>(`/containers/${containerId}/files/write`, { method: 'POST', body: JSON.stringify({ path, content }) }),
    createContainerFolder: (containerId: string, path: string) =>
        request<{ message: string }>(`/containers/${containerId}/files/folder`, { method: 'POST', body: JSON.stringify({ path }) }),
    deleteContainerFile: (containerId: string, path: string) =>
        request<{ message: string }>(`/containers/${containerId}/files/delete?path=${encodeURIComponent(path)}`, { method: 'DELETE' }),
    uploadContainerFile: async (containerId: string, path: string, file: File, onProgress?: (progress: number) => void) => {
        const t = get(token);
        const CHUNK_SIZE = 5 * 1024 * 1024; // 5MB chunks
        const apiUrl = import.meta.env.VITE_API_URL || 'http://localhost:3000';
        
        // For files smaller than chunk size, use simple upload
        if (file.size <= CHUNK_SIZE) {
            const formData = new FormData();
            formData.append('file', file);
            formData.append('path', path);

            const response = await fetch(`${apiUrl}/containers/${containerId}/files/upload`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${t}`
                },
                body: formData
            });

            if (!response.ok) {
                const data = await response.json().catch(() => ({}));
                throw new Error(data.error || 'Upload failed');
            }
            onProgress?.(100);
            return response.json();
        }
        
        // For large files, use chunked upload
        const totalChunks = Math.ceil(file.size / CHUNK_SIZE);
        const uploadId = crypto.randomUUID();
        
        for (let chunkIndex = 0; chunkIndex < totalChunks; chunkIndex++) {
            const start = chunkIndex * CHUNK_SIZE;
            const end = Math.min(start + CHUNK_SIZE, file.size);
            const chunk = file.slice(start, end);
            
            const formData = new FormData();
            formData.append('chunk', chunk);
            formData.append('path', path);
            formData.append('uploadId', uploadId);
            formData.append('chunkIndex', chunkIndex.toString());
            formData.append('totalChunks', totalChunks.toString());
            formData.append('fileName', file.name);
            formData.append('fileSize', file.size.toString());
            
            const response = await fetch(`${apiUrl}/containers/${containerId}/files/upload-chunk`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${t}`
                },
                body: formData
            });

            if (!response.ok) {
                const data = await response.json().catch(() => ({}));
                throw new Error(data.error || `Chunk ${chunkIndex + 1}/${totalChunks} upload failed`);
            }
            
            onProgress?.(Math.round(((chunkIndex + 1) / totalChunks) * 100));
        }
        
        return { message: 'Upload complete' };
    },

    // Admin: All containers
    listAllContainers: () => request<Container[]>('/admin/containers'),

    // Flakes
    listFlakes: () => request<import('./types').Flake[]>('/flakes'),
    getFlake: (id: string) => request<import('./types').FlakeWithVariables>(`/flakes/${id}`),
    createFlake: (data: any) => request<import('./types').FlakeWithVariables>('/flakes', { method: 'POST', body: JSON.stringify(data) }),
    importFlake: (flakeJson: any) => request<import('./types').FlakeWithVariables>('/flakes/import', { method: 'POST', body: JSON.stringify({ flakeJson }) }),
    deleteFlake: (id: string) => request<void>(`/flakes/${id}`, { method: 'DELETE' }),
    exportFlake: (id: string) => request<any>(`/flakes/${id}/export`),
};

export function createWebSocket(containerId: string): WebSocket {
    const t = get(token);
    const wsUrl = (import.meta.env.VITE_API_URL || 'http://localhost:3000')
        .replace('http', 'ws');
    return new WebSocket(`${wsUrl}/ws/containers/${containerId}/logs?token=${t}`);
}

export function createStatsWebSocket(containerId: string): WebSocket {
    const t = get(token);
    const wsUrl = (import.meta.env.VITE_API_URL || 'http://localhost:3000')
        .replace('http', 'ws');
    return new WebSocket(`${wsUrl}/ws/containers/${containerId}/stats?token=${t}`);
}
