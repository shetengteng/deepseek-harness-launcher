<template>
  <main class="flex-1 flex items-center justify-center bg-background">
    <section class="flex flex-col items-center gap-[18px] text-center">
      <span
        class="h-8 w-8 rounded-full border-2 border-border border-t-primary animate-spin"
        aria-hidden="true"
      />
      <p class="text-sm font-medium">正在启动 dsh...</p>
      <p
        class="rounded-sm border bg-secondary px-[9px] py-1 font-mono text-[11px] text-muted-foreground"
      >
        <span class="text-success">$</span> node --expose-internals dsh web
      </p>
    </section>
  </main>
</template>
