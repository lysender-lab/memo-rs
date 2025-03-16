(function () {
  if (!window.X_LOGIN_EVENTS) {
    window.X_LOGIN_EVENTS = true;

    function loginLoading() {
      const btn = document.getElementById('btn-login');
      if (btn) {
        btn.classList.add('is-loading');
      }
    }

    document.addEventListener('submit', (e) => {
      if (e.target.closest('#login-form')) {
        loginLoading();
      }
    });
  }
})();
