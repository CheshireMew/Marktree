import { createApp } from 'vue'

import App from './App.vue'
import { i18n } from './i18n'
import './styles.css'
import './styles/shell.css'
import './styles/editor.css'
import './styles/git.css'
import './styles/diff.css'
import './styles/overlays.css'
import './styles/dialogs.css'
import './styles/conflicts.css'
import './styles/account-dialogs.css'
import './styles/responsive.css'

createApp(App).use(i18n).mount('#app')
