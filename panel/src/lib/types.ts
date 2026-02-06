export interface User {
    id: string;
    username: string;
    email?: string;
    avatarUrl?: string;
    roleId?: string;
    roleName?: string;
    permissions: Record<string, boolean>;
}

export interface Role {
    id: string;
    name: string;
    permissions: Record<string, boolean>;
}

export interface ResourceLimits {
    memory: number;
    cpu: number;
    disk: number;
    swap?: number;
    io?: number;
}

export interface ContainerAllocation {
    id: string;
    allocationId?: string;
    ip: string;
    port: number;
    internalPort: number;
    protocol: string;
    isPrimary: boolean;
}

export interface Container {
    id: string;
    userId: string;
    daemonId: string;
    name: string;
    image: string;
    startupScript?: string;
    status: string;
    sftpUser?: string;
    sftpPass?: string;
    allocations?: ContainerAllocation[];
    allocationIp?: string;
    allocationPort?: number;
    memoryLimit?: number;
    cpuLimit?: number;
    diskLimit?: number;
    swapLimit?: number;
    ioWeight?: number;
    createdAt: string;
    updatedAt: string;
}

export interface Daemon {
    id: string;
    name: string;
    host: string;
    port: number;
    apiKey: string;
    location?: string;
    secure?: boolean;
    totalMemory?: number;
    totalCpu?: number;
    totalDisk?: number;
    usedMemory?: number;
    usedCpu?: number;
    usedDisk?: number;
    createdAt: string;
    updatedAt: string;
}

export interface Allocation {
    id: string;
    daemonId: string;
    ip: string;
    port: number;
    protocol: string;
    containerId?: string;
    createdAt: string;
    updatedAt: string;
}

export interface ContainerPort {
    id: string;
    containerId: string;
    hostPort: number;
    containerPort: number;
    protocol: string;
    createdAt: string;
}

export interface ServerTemplate {
    id: string;
    name: string;
    description?: string;
    image: string;
    defaultMemory: number;
    defaultCpu: number;
    defaultDisk: number;
    minMemory: number;
    minCpu: number;
    minDisk: number;
    maxMemory: number;
    maxCpu: number;
    maxDisk: number;
    startupScript?: string;
    createdAt: string;
    updatedAt: string;
}

export interface Flake {
    id: string;
    name: string;
    slug: string;
    author: string | null;
    description: string | null;
    dockerImage: string;
    startupCommand: string;
    configFiles: Record<string, unknown>;
    startupDetection: string | null;
    installScript: string | null;
    installContainer: string | null;
    installEntrypoint: string | null;
    features: string[];
    fileDenylist: string[];
    createdAt: string;
    updatedAt: string;
}

export interface FlakeVariable {
    id: string;
    flakeId: string;
    name: string;
    description: string | null;
    envVariable: string;
    defaultValue: string | null;
    rules: string | null;
    userViewable: boolean;
    userEditable: boolean;
    sortOrder: number;
    createdAt: string;
}

export interface FlakeWithVariables extends Flake {
    variables: FlakeVariable[];
}

export interface ContainerVariableInfo {
    envVariable: string;
    name: string;
    description: string | null;
    defaultValue: string | null;
    value: string;
    userViewable: boolean;
    userEditable: boolean;
    rules: string | null;
    sortOrder: number;
}

export interface ContainerStartupInfo {
    startupScript: string | null;
    variables: ContainerVariableInfo[];
}
