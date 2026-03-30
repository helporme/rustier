<script lang="ts">
  import { browser } from "$app/environment";
  import { greet } from "./greeting.rs";
</script>

<svelte:head>
  <title>Home</title>
  <meta name="description" content="Rust curse demo" />
</svelte:head>

<section>{greet(browser ? "%browser user%" : "%server user%")}</section>
