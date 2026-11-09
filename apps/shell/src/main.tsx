import '@fontsource/anta/400.css';
import '@fontsource/cousine/400.css';
import '@fontsource/cousine/700.css';
import '@nexus/ui/theme.css';
import './styles.css';
import { render } from 'solid-js/web';
import { App } from './components/App';

render(() => <App />, document.getElementById('root')!);
