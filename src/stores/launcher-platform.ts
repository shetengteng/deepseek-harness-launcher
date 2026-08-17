let hostPlatformArch: { platform: string; arch: string } | null = null;

export function setPlatformArch(platform: string, arch: string): void {
  hostPlatformArch = { platform, arch };
}

export function detectPlatformArch(): { platform: string; arch: string } {
  if (!hostPlatformArch) {
    throw new Error("platform/arch unknown: fetchStatus not called yet");
  }
  return hostPlatformArch;
}
