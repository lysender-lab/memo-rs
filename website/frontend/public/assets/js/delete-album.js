(function () {
  if (!window.X_DELETE_ALBUM_EVENTS) {
    window.X_DELETE_ALBUM_EVENTS = true;

    document.addEventListener('click', (e) => {
      if (e.target.closest('#btn-delete-album')) {
        swal({
          title: 'Are you sure?',
          text: 'Are you sure that you want to delete this album?',
          icon: 'warning',
          buttons: true,
          dangerMode: true,
        }).then((willDelete) => {
          if (willDelete) {
            const btn = document.getElementById('btn-delete-album');
            if (btn) {
              htmx.trigger(btn, 'confirmed');
            }
          }
        });
      }
    });
  }
})();
