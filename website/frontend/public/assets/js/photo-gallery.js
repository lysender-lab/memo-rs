(function () {
  if (!window.X_GALLERY_EVENTS) {
    window.X_GALLERY_EVENTS = true;

    function handlePhotoDeleted() {
      const currentNode = document.querySelector(
        '#photos-count-w .current-count',
      );
      const totalNode = document.querySelector(
        '#photos-count-w .total-records',
      );

      if (currentNode && totalNode) {
        const current = Number.parseInt(
          currentNode.innerHTML.toString().trim(),
          10,
        );
        const total = Number.parseInt(
          totalNode.innerHTML.toString().trim(),
          10,
        );

        currentNode.innerText = current - 1;
        totalNode.innerText = total - 1;
      }
    }

    htmx.onLoad(function () {
      var lightbox = new PhotoSwipeLightbox({
        gallery: '#photo-gallery',
        children: '.photo-item-src',
        // dynamic import is not supported in UMD version
        pswpModule: PhotoSwipe,

        showHideAnimationType: 'none',
        showHideDuration: false,
      });

      lightbox.init();
    });

    htmx.on('PhotoDeletedEvent', handlePhotoDeleted);
  }
})();
