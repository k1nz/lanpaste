import { createApp } from "vue";
import App from "./App.vue";
import { detectLocale, i18n, persistLocale } from "./i18n";
import "./style.css";

const app = createApp(App);
const locale = detectLocale();
i18n.global.locale.value = locale;
persistLocale(locale);
app.use(i18n);
app.mount("#app");
