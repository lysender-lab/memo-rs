function handleFileDeleted() {
  const currentNode = document.querySelector('#files-count-w .current-count');
  const totalNode = document.querySelector('#files-count-w .total-records');

  if (currentNode && totalNode) {
    const current = Number.parseInt(
      currentNode.innerHTML.toString().trim(),
      10,
    );
    const total = Number.parseInt(totalNode.innerHTML.toString().trim(), 10);

    currentNode.innerText = current - 1;
    totalNode.innerText = total - 1;
  }
}

htmx.on('FileDeletedEvent', handleFileDeleted);
