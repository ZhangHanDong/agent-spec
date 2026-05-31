(() => {
    const darkThemes = ['ayu', 'navy', 'coal'];
    const lightThemes = ['light', 'rust'];

    function themeIsLight() {
        const classList = document.getElementsByTagName('html')[0].classList;
        for (const cssClass of classList) {
            if (darkThemes.includes(cssClass)) {
                return false;
            }
        }
        return true;
    }

    function prepareMermaidBlocks() {
        const blocks = document.querySelectorAll('pre > code.language-mermaid, pre > code.mermaid');
        blocks.forEach((code, index) => {
            const pre = code.parentElement;
            if (!pre) {
                return;
            }

            const diagram = document.createElement('div');
            diagram.className = 'mermaid';
            diagram.dataset.mdbookMermaid = String(index);
            diagram.textContent = code.textContent;
            pre.replaceWith(diagram);
        });
    }

    function installThemeRefresh(lastThemeWasLight) {
        for (const darkTheme of darkThemes) {
            const button = document.getElementById(darkTheme);
            if (button) {
                button.addEventListener('click', () => {
                    if (lastThemeWasLight) {
                        window.location.reload();
                    }
                });
            }
        }

        for (const lightTheme of lightThemes) {
            const button = document.getElementById(lightTheme);
            if (button) {
                button.addEventListener('click', () => {
                    if (!lastThemeWasLight) {
                        window.location.reload();
                    }
                });
            }
        }
    }

    function renderMermaid() {
        if (!window.mermaid) {
            console.error('Mermaid runtime was not loaded.');
            return;
        }

        const lastThemeWasLight = themeIsLight();
        const theme = lastThemeWasLight ? 'default' : 'dark';

        prepareMermaidBlocks();
        window.mermaid.initialize({
            startOnLoad: false,
            theme,
            securityLevel: 'strict',
        });

        if (typeof window.mermaid.run === 'function') {
            window.mermaid.run({ querySelector: '.mermaid' });
        } else {
            window.mermaid.init(undefined, document.querySelectorAll('.mermaid'));
        }

        installThemeRefresh(lastThemeWasLight);
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', renderMermaid);
    } else {
        renderMermaid();
    }
})();
