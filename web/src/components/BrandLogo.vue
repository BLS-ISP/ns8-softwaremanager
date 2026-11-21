<script setup lang="ts">
let logoInstanceCounter = 0
const uid = ++logoInstanceCounter
const shieldId = `swm-shield-${uid}`
const leftFaceId = `swm-cube-left-${uid}`
const rightFaceId = `swm-cube-right-${uid}`

withDefaults(
  defineProps<{
    size?: 'sm' | 'md' | 'lg'
  }>(),
  {
    size: 'md',
  },
)
</script>

<template>
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 64 64"
    role="img"
    aria-label="NS8 SoftwareManager Logo"
    data-testid="brand-logo"
    class="shrink-0 select-none"
    :class="{
      'h-7 w-7': size === 'sm',
      'h-9 w-9': size === 'md',
      'h-14 w-14': size === 'lg',
    }"
  >
    <defs>
      <linearGradient :id="shieldId" x1="8" y1="4" x2="56" y2="60" gradientUnits="userSpaceOnUse">
        <stop offset="0%" stop-color="#06b6d4" />
        <stop offset="50%" stop-color="#0284c7" />
        <stop offset="100%" stop-color="#2563eb" />
      </linearGradient>
      <linearGradient :id="leftFaceId" x1="20" y1="23" x2="31" y2="45" gradientUnits="userSpaceOnUse">
        <stop offset="0%" stop-color="#f0f9ff" />
        <stop offset="100%" stop-color="#bae6fd" />
      </linearGradient>
      <linearGradient :id="rightFaceId" x1="33" y1="23" x2="44" y2="45" gradientUnits="userSpaceOnUse">
        <stop offset="0%" stop-color="#bae6fd" />
        <stop offset="100%" stop-color="#38bdf8" />
      </linearGradient>
    </defs>
    <!-- Protective Update Shield -->
    <path
      d="M32 4L55 13.5V30C55 45.2 45 56 32 60C19 56 9 45.2 9 30V13.5L32 4Z"
      :fill="`url(#${shieldId})`"
    />
    <path
      d="M32 6.8L52.4 15.2V30C52.4 43.5 43.4 53.3 32 57.1C20.6 53.3 11.6 43.5 11.6 30V15.2L32 6.8Z"
      fill="none"
      stroke="#ffffff"
      stroke-opacity="0.28"
      stroke-width="1.25"
    />
    <!-- Layered Isometric Software Package Cube -->
    <path d="M32 14L44.5 21.2L32 28.4L19.5 21.2L32 14Z" fill="#ffffff" />
    <path
      d="M25.8 17.6L38.2 24.8"
      fill="none"
      stroke="#0284c7"
      stroke-width="1.8"
      stroke-linecap="round"
    />
    <path d="M19.5 23.8L30.8 30.3V44.8L19.5 38.3V23.8Z" :fill="`url(#${leftFaceId})`" />
    <path d="M44.5 23.8L33.2 30.3V44.8L44.5 38.3V23.8Z" :fill="`url(#${rightFaceId})`" />
    <!-- Ed25519 Verified Update Badge -->
    <circle cx="45.5" cy="44.5" r="9.5" fill="#0f172a" fill-opacity="0.28" />
    <circle cx="45.5" cy="44.5" r="8" fill="#ffffff" />
    <path
      d="M41.8 44.6L44.4 47.2L49.4 42"
      fill="none"
      stroke="#2563eb"
      stroke-width="2.4"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  </svg>
</template>
