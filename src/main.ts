import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { initializeLocale } from "./lib/i18n";
import { useThemeStore } from "./stores/theme";
import "./styles.css";

const app = createApp(App);
const pinia = createPinia();
app.use(pinia);
initializeLocale();
void useThemeStore(pinia).initialize();
app.mount("#app");
