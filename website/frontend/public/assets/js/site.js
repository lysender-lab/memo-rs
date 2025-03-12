(function () {
  if (!window.X_BEFORE_SWAP) {
    window.X_BEFORE_SWAP = true;

    document.body.addEventListener('htmx:beforeSwap', function (evt) {
      // Allow some status codes to be handled
      const codes = [400, 401, 403, 404, 422, 500];

      if (codes.includes(evt.detail.xhr.status)) {
        evt.detail.shouldSwap = true;
        evt.detail.isError = false;
      }

      const successCodes = [200, 201, 204];
      if (successCodes.includes(evt.detail.xhr.status)) {
        evt.detail.isError = false;
        evt.detail.shouldSwap = true;
      }
    });

    document.body.addEventListener('LightThemeSetEvent', function () {
      switchTheme('light');
    });

    document.body.addEventListener('DarkThemeSetEvent', function () {
      switchTheme('dark');
    });
  }

  function switchTheme(theme) {
    const root = document.documentElement;
    if (theme === 'dark') {
      root.dataset.theme = 'dark';
    } else {
      root.dataset.theme = 'light';
    }
  }
})();
