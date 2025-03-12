(function () {
  if (!window.X_NAV_EVENTS) {
    window.X_NAV_EVENTS = true;

    document.addEventListener('click', (e) => {
      if (e.target.closest('#main-menu-burger')) {
        console.log('main menu burger clicked');
        toggleLogoutMenu();
      }
    });

    document.addEventListener('click', (e) => {
      if (e.target.closest('#btn-logout')) {
        toggleLogoutMenu();
      }
    });

    function toggleLogoutMenu() {
      const menu = document.getElementById('main-menu');
      if (menu) {
        menu.classList.toggle('is-active');
      }

      const burger = document.getElementById('main-menu-burger');
      if (burger) {
        burger.classList.toggle('is-active');
      }
    }
  }
})();
