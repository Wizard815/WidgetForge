import "./hub.css";
import { mount } from "svelte";
import App from "./App.svelte";
import { init } from "./store.svelte";

init();
mount(App, { target: document.getElementById("app")! });
