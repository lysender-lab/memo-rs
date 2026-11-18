document.addEventListener('change', (event) => {
  if (event.target.id !== 'note-revision-id') return;

  const revision = event.target.value;
  window.location.href =
    window.location.pathname +
    (revision ? `?revision=${encodeURIComponent(revision)}` : '');
});
