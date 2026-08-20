import { useContext, useEffect, type ReactNode } from 'react';
import {
  ThemeContext,
  removeBase,
  useLang,
  useLocation,
  usePages,
  useSite,
  useVersion,
  withBase,
} from '@rspress/core/runtime';
import {
  findPageByRoutePath,
  resolveVersionRoutePath,
} from '../../version-routing';

type RootProps = {
  children: ReactNode;
};

export function Root({ children }: RootProps) {
  const location = useLocation();
  const currentLang = useLang();
  const currentVersion = useVersion();
  const { pages } = usePages();
  const { site } = useSite();
  const { theme } = useContext(ThemeContext);

  useEffect(() => {
    try {
      localStorage.setItem('themeMode', theme);
    } catch {
      // Storage can be unavailable in privacy-restricted browsing contexts.
    }

    document
      .querySelector<HTMLMetaElement>('meta[name="theme-color"]')
      ?.setAttribute('content', theme === 'dark' ? '#101118' : '#f7f7f8');
  }, [theme]);

  useEffect(() => {
    const sidebarQuery = window.matchMedia('(max-width: 768px)');
    const outlineQuery = window.matchMedia('(max-width: 1279px)');
    const navigationPanelId = 'rspress-primary-navigation';
    let wasNavigationOpen = false;
    const focusableSelector = [
      'a[href]',
      'button:not([disabled])',
      'input:not([disabled])',
      'select:not([disabled])',
      'textarea:not([disabled])',
      'summary',
      '[tabindex]:not([tabindex="-1"])',
    ].join(', ');

    const isVisible = (element: HTMLElement) => {
      const styles = getComputedStyle(element);
      const bounds = element.getBoundingClientRect();
      return (
        styles.display !== 'none' &&
        styles.visibility !== 'hidden' &&
        bounds.width > 0 &&
        bounds.height > 0
      );
    };

    const visibleNavigationTrigger = () =>
      Array.from(
        document.querySelectorAll<HTMLElement>('.rp-nav-hamburger'),
      ).find(isVisible) ?? null;

    const synchronizePanel = (
      panel: HTMLElement | null,
      trigger: HTMLElement | null,
      hidden: boolean,
      id: string,
    ) => {
      if (!panel) return;

      const wasHidden = panel.inert;
      panel.id ||= id;
      panel.inert = hidden;
      if (hidden) panel.setAttribute('aria-hidden', 'true');
      else panel.removeAttribute('aria-hidden');

      if (trigger) {
        trigger.setAttribute('aria-controls', panel.id);
        trigger.setAttribute('aria-expanded', String(!hidden));
      }

      if (hidden && !wasHidden && panel.contains(document.activeElement)) {
        trigger?.focus();
      } else if (!hidden && wasHidden && trigger === document.activeElement) {
        requestAnimationFrame(() => {
          panel.querySelector<HTMLElement>(focusableSelector)?.focus();
        });
      }
    };

    const synchronizeSidebarGroups = () => {
      document
        .querySelectorAll<HTMLElement>('.rp-sidebar-group')
        .forEach((group, index) => {
          const panel = group.nextElementSibling;
          if (!(panel instanceof HTMLElement)) return;

          const hidden = panel.style.gridTemplateRows === '0fr';
          panel.id ||= `rspress-sidebar-group-${index}-panel`;
          panel.inert = hidden;
          if (hidden) panel.setAttribute('aria-hidden', 'true');
          else panel.removeAttribute('aria-hidden');

          if (!group.matches('a[href]')) {
            group.setAttribute('role', 'button');
            group.tabIndex = 0;
          }
          group.setAttribute('aria-controls', panel.id);
          group.setAttribute('aria-expanded', String(!hidden));

          if (hidden && panel.contains(document.activeElement)) group.focus();
        });
    };

    const synchronizeNavigation = (isChinese: boolean) => {
      const navigationPanel =
        document.querySelector<HTMLElement>('.rp-nav-screen');
      const isOpen = Boolean(
        navigationPanel?.classList.contains('rp-nav-screen--open') ||
        document.querySelector(
          '.rp-nav-hamburger__sm.rp-nav-hamburger--active',
        ),
      );
      const navigationTrigger = visibleNavigationTrigger();

      document
        .querySelectorAll<HTMLElement>('.rp-nav-hamburger')
        .forEach((trigger) => {
          trigger.setAttribute('aria-controls', navigationPanelId);
          trigger.setAttribute('aria-expanded', String(isOpen));
          trigger.setAttribute(
            'aria-label',
            isOpen
              ? isChinese
                ? '关闭主导航'
                : 'Close navigation'
              : isChinese
                ? '打开主导航'
                : 'Open navigation',
          );
        });

      if (navigationPanel) {
        navigationPanel.id = navigationPanelId;
        navigationPanel.inert = !isOpen;
        if (isOpen) navigationPanel.removeAttribute('aria-hidden');
        else navigationPanel.setAttribute('aria-hidden', 'true');
      }

      if (
        isOpen &&
        !wasNavigationOpen &&
        navigationPanel &&
        navigationTrigger === document.activeElement
      ) {
        requestAnimationFrame(() => {
          navigationPanel
            .querySelector<HTMLElement>(focusableSelector)
            ?.focus();
        });
      } else if (
        !isOpen &&
        wasNavigationOpen &&
        (!document.activeElement?.isConnected ||
          document.activeElement === document.body)
      ) {
        requestAnimationFrame(() => navigationTrigger?.focus());
      }

      wasNavigationOpen = isOpen;
    };

    const syncDocumentationControls = () => {
      const isDark = document.documentElement.classList.contains('dark');
      const isChinese = document.documentElement.lang.startsWith('zh');

      synchronizeNavigation(isChinese);
      synchronizeSidebarGroups();

      document
        .querySelectorAll<HTMLElement>('.rp-switch-appearance')
        .forEach((toggle) => {
          toggle.setAttribute('role', 'button');
          toggle.tabIndex = 0;
          toggle.setAttribute(
            'aria-label',
            isDark
              ? isChinese
                ? '切换到浅色主题'
                : 'Switch to light theme'
              : isChinese
                ? '切换到深色主题'
                : 'Switch to dark theme',
          );
        });

      document
        .querySelectorAll<HTMLElement>('.rp-search-button--mobile')
        .forEach((searchButton) => {
          searchButton.setAttribute('role', 'button');
          searchButton.setAttribute('aria-haspopup', 'dialog');
          searchButton.setAttribute(
            'aria-label',
            isChinese ? '搜索文档' : 'Search documentation',
          );
          searchButton.tabIndex = 0;
        });

      const sidebar = document.querySelector<HTMLElement>(
        '.rp-doc-layout__sidebar',
      );
      const sidebarTrigger = document.querySelector<HTMLElement>(
        '.rp-sidebar-menu__left',
      );
      synchronizePanel(
        sidebar,
        sidebarTrigger,
        sidebarQuery.matches &&
          !sidebar?.classList.contains('rp-doc-layout__sidebar--open'),
        'rspress-documentation-sidebar',
      );

      const outline = document.querySelector<HTMLElement>(
        '.rp-doc-layout__outline',
      );
      const outlineTrigger = document.querySelector<HTMLElement>(
        '.rp-sidebar-menu__right',
      );
      synchronizePanel(
        outline,
        outlineTrigger,
        outlineQuery.matches &&
          !outline?.classList.contains('rp-doc-layout__outline--open'),
        'rspress-documentation-outline',
      );
    };

    const handleDocumentationKeyboard = (event: KeyboardEvent) => {
      if (!(event.target instanceof HTMLElement)) return;

      const sidebarGroup = event.target.closest<HTMLElement>(
        '.rp-sidebar-group[role="button"]',
      );
      if (
        sidebarGroup &&
        (event.key === 'Enter' || event.key === ' ') &&
        !event.isComposing
      ) {
        event.preventDefault();
        event.stopPropagation();
        sidebarGroup.click();
        return;
      }

      const keyboardControl = event.target.closest<HTMLElement>(
        '.rp-switch-appearance, .rp-search-button--mobile',
      );
      if (keyboardControl && (event.key === 'Enter' || event.key === ' ')) {
        event.preventDefault();
        event.stopPropagation();
        keyboardControl.click();
        return;
      }

      if (event.key === 'Escape') {
        const openNavigation = document.querySelector<HTMLElement>(
          '.rp-nav-screen--open',
        );
        const openSidebar = document.querySelector<HTMLElement>(
          '.rp-doc-layout__sidebar--open',
        );
        const openOutline = document.querySelector<HTMLElement>(
          '.rp-doc-layout__outline--open',
        );

        if (openNavigation) {
          event.preventDefault();
          event.stopPropagation();
          const navigationTrigger = visibleNavigationTrigger();
          navigationTrigger?.click();
          requestAnimationFrame(() => navigationTrigger?.focus());
          return;
        }

        if (openOutline) {
          event.preventDefault();
          event.stopPropagation();
          document
            .querySelector<HTMLElement>('.rp-sidebar-menu__right')
            ?.click();
          return;
        }

        if (openSidebar) {
          event.preventDefault();
          event.stopPropagation();
          document
            .querySelector<HTMLElement>('.rp-sidebar-menu__mask')
            ?.click();
          return;
        }
      }

      if (
        event.key !== 'Enter' ||
        event.isComposing ||
        event.defaultPrevented
      ) {
        return;
      }

      // Rspress 2.0.19 listens for Enter on document even when search is
      // closed. Let interactive controls handle Enter normally, then stop the
      // event before that inactive search listener can read an empty result.
      if (event.target.closest('.rp-search-panel')) return;

      const interactive = event.target.closest(
        [
          'a[href]',
          'button',
          'input',
          'select',
          'textarea',
          'summary',
          '[role="button"]',
          '[role="link"]',
          '[role="menuitem"]',
          '[role="tab"]',
        ].join(', '),
      );

      if (interactive) event.stopPropagation();
    };

    syncDocumentationControls();
    const themeObserver = new MutationObserver(syncDocumentationControls);
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class', 'lang'],
    });
    const modalContainer = document.querySelector('#__rspress_modal_container');
    if (modalContainer) {
      themeObserver.observe(modalContainer, {
        attributes: true,
        attributeFilter: ['class', 'style'],
        childList: true,
        subtree: true,
      });
    }
    const navigation = document.querySelector('.rp-nav');
    if (navigation) {
      themeObserver.observe(navigation, {
        attributes: true,
        attributeFilter: ['class', 'style'],
        childList: true,
        subtree: true,
      });
    }
    const documentationLayout = document.querySelector(
      '.rp-doc-layout__container',
    );
    if (documentationLayout) {
      themeObserver.observe(documentationLayout, {
        attributes: true,
        attributeFilter: ['class', 'style'],
        childList: true,
        subtree: true,
      });
    }

    sidebarQuery.addEventListener('change', syncDocumentationControls);
    outlineQuery.addEventListener('change', syncDocumentationControls);

    document.body.addEventListener('keydown', handleDocumentationKeyboard);
    return () => {
      themeObserver.disconnect();
      sidebarQuery.removeEventListener('change', syncDocumentationControls);
      outlineQuery.removeEventListener('change', syncDocumentationControls);
      document.body.removeEventListener('keydown', handleDocumentationKeyboard);
    };
  }, [location.pathname]);

  useEffect(() => {
    // Rspress maps every version item to the current path even when the target
    // version does not publish that page. Keep exact matches and fall back to
    // the target version's localized homepage for version-exclusive content.
    const currentHref = `${window.location.pathname}${window.location.search}${window.location.hash}`;
    const currentPage = findPageByRoutePath(
      pages,
      removeBase(window.location.pathname),
    );
    if (!currentPage) return;

    const versions = new Set(site.multiVersion.versions || []);
    const candidates = document.querySelectorAll<HTMLAnchorElement>(
      [
        '.rp-hover-group__item a[aria-label]',
        '.rp-nav-screen-versions-group__item',
      ].join(', '),
    );

    candidates.forEach((link) => {
      const label = link.getAttribute('aria-label') ?? link.textContent?.trim();
      if (!label || !versions.has(label)) return;

      if (label === currentVersion) {
        link.href = currentHref;
        link.setAttribute('aria-current', 'page');
        return;
      }

      link.href = withBase(
        resolveVersionRoutePath(pages, currentPage, label, {
          defaultLang: site.lang || '',
          defaultVersion: site.multiVersion.default || '',
        }),
      );
      link.removeAttribute('aria-current');
    });
  }, [
    currentLang,
    currentVersion,
    location.pathname,
    location.search,
    location.hash,
    pages,
    site.lang,
    site.multiVersion.default,
    site.multiVersion.versions,
  ]);

  return <>{children}</>;
}
