<script lang="ts">
  import { greet } from "./greeting.rs";
</script>

<svelte:head>
  <title>Home</title>
  <meta name="description" content="Rust curse demo" />
</svelte:head>

<section>{greet("%user%")}</section>
