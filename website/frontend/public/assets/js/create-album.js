(function () {
  if (!window.X_CREATE_ALBUM_EVENTS) {
    window.X_CREATE_ALBUM_EVENTS = true;

    /**
     * Converts a label to a url friendly name
     *
     * @param {string} label
     * @return {string}
     */
    function labelToName(label) {
      if (label.length === 0) {
        return '';
      }

      return label
        .toLowerCase()
        .replace(/\s+/g, '-')
        .replace(/[^a-z0-9-]/g, '');
    }

    document.addEventListener('keyup', (e) => {
      if (e.target.closest('#create-album-label')) {
        const name = labelToName(e.target.value.toString());
        document.getElementById('create-album-name').value = name;
      }
    });
  }
})();
