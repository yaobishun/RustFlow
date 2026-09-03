<script setup lang="ts">
defineProps<{ label: string; value: string|number; note?: string; tone?: 'blue'|'cyan'|'amber'|'red' }>()
</script>
<template><div class="metric-card" :class="`tone-${tone || 'blue'}`"><div class="metric-label">{{ label }}</div><div class="metric-value">{{ value }}</div><div v-if="note" class="metric-note">{{ note }}</div></div></template>

