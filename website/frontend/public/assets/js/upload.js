function handleFilesSelect(e) {
  const files = e.target.files;
  if (files.length > 0) {
    const label = document.getElementById('selected-files-label');
    if (label) {
      label.innerText = files.length + ' file(s) selected';
    }

    const container =
      document.getElementById('files-input-w') ||
      document.getElementById('photos-input-w');
    if (container) {
      container.classList.add('is-success');
    }
  }
}

function showUploadFinished(title) {
  const elem =
    document.getElementById('h-uploading-files') ||
    document.getElementById('h-uploading-photos');
  if (elem) {
    elem.innerHTML = `${title} finished`;
  }
}

function showUploadMore() {
  const container = document.getElementById('upload-more-w');
  if (container) {
    container.classList.remove('is-hidden');
  }
}

function createDomElement(html) {
  const template = document.createElement('template');
  template.innerHTML = html.trim();
  return template.content.firstChild;
}

function getFileExtension(name) {
  const normalized = (name || '').toLowerCase();
  const dotIndex = normalized.lastIndexOf('.');
  if (dotIndex === -1) {
    return '';
  }

  return normalized.slice(dotIndex);
}

function filterFilesByAllowedExtensions(fileInput, files) {
  const allowedExtsRaw = fileInput?.dataset?.allowedExts || '';
  if (!allowedExtsRaw) {
    return {
      allowed: files,
      rejected: [],
    };
  }

  const allowedSet = new Set(
    allowedExtsRaw
      .split(',')
      .map((item) => item.trim().toLowerCase())
      .filter((item) => item.startsWith('.')),
  );

  const allowed = [];
  const rejected = [];

  for (const file of files) {
    const ext = getFileExtension(file.name);
    if (allowedSet.has(ext)) {
      allowed.push(file);
    } else {
      rejected.push(file.name);
    }
  }

  return {
    allowed,
    rejected,
  };
}

function appendValidationWarning(errorsContainer, rejected) {
  if (!errorsContainer || rejected.length === 0) {
    return;
  }

  const preview = rejected.slice(0, 5).join(', ');
  const suffix = rejected.length > 5 ? '...' : '';
  const msg = `<p class="has-text-danger">Skipped unsupported files: ${preview}${suffix}</p>`;
  errorsContainer.appendChild(createDomElement(msg));
}

function chunkArray(items, chunkSize) {
  const chunks = [];

  for (let i = 0; i < items.length; i += chunkSize) {
    chunks.push(items.slice(i, i + chunkSize));
  }

  return chunks;
}

function startUpload() {
  uploadFiles()
    .then(() => {
      showUploadFinished('Upload');
      showUploadMore();
    })
    .catch((_err) => {
      showUploadFinished('Upload');
      showUploadMore();
    });
}

async function prepareUpload(url, token, file) {
  const config = {
    headers: {
      'Content-Type': 'application/json',
    },
  };

  const data = {
    filename: file.name,
    content_type: file.type || 'application/octet-stream',
    size: file.size,
    token,
  };

  const res = await axios.post(url, data, config);
  return {
    nextToken: res.headers['x-next-token'],
    data: res.data,
  };
}

async function commitUpload(url, token, uploadToken) {
  const config = {
    headers: {
      'Content-Type': 'application/x-www-form-urlencoded',
    },
  };

  const body = new URLSearchParams({
    token,
    upload_token: uploadToken,
  });

  const res = await axios.post(url, body, config);
  return {
    nextToken: res.headers['x-next-token'],
    html: res.data,
  };
}

async function uploadPhoto(url, file) {
  const config = {
    headers: {
      'Content-Type': file.type || 'application/octet-stream',
    },
  };

  await axios.put(url, file, config);
}

async function remoteUploadPhoto(prepareUrl, commitUrl, token, file) {
  const prepared = await prepareUpload(prepareUrl, token, file);
  const remoteUploadUrl = prepared.data.url;
  const uploadToken = prepared.data.token;

  await uploadPhoto(remoteUploadUrl, file);

  return await commitUpload(commitUrl, token, uploadToken);
}

async function uploadFiles() {
  // Match chunks with server's core (2)
  const CHUNK_SIZE = 2;
  const form =
    document.getElementById('upload-files-form') ||
    document.getElementById('upload-photos-form');
  const filesInput =
    document.getElementById('files-input') ||
    document.getElementById('photos-input');
  const tokenInput =
    document.getElementById('upload-files-token') ||
    document.getElementById('upload-photos-token');
  const prepareUploadInput = document.getElementById('prepare-upload-url');
  const galleryContainer =
    document.getElementById('uploaded-items') ||
    document.getElementById('photo-gallery');
  const uploadContainer =
    document.getElementById('files-input-w') ||
    document.getElementById('photos-input-w');
  const progressContainer = document.getElementById('upload-progress-w');
  const errorsContainer = document.getElementById('progress-errors-w');
  const successElement = document.getElementById('progress-uploaded-count');
  const failedElement = document.getElementById('progress-failed-count');

  if (
    !form ||
    !uploadContainer ||
    !filesInput ||
    !tokenInput ||
    !prepareUploadInput ||
    !galleryContainer ||
    !progressContainer ||
    !errorsContainer ||
    !successElement ||
    !failedElement
  ) {
    return;
  }

  const files = filesInput.files;
  const prepareUploadUrl = prepareUploadInput.value;
  const commitUploadUrl = form.action;

  // Token will change on every upload batch
  let token = tokenInput.value.toString();

  if (files.length === 0) {
    alert('Please select files to upload');
    return;
  }

  const filesArray = Array.from(files);
  const filtered = filterFilesByAllowedExtensions(filesInput, filesArray);

  if (filtered.allowed.length === 0) {
    alert('No supported files selected');
    return;
  }

  const totalFiles = filtered.allowed.length;
  let uploadedCount = 0;
  let failedCount = 0;

  const updateOverallProgress = () => {
    const overallProgress = Math.round((uploadedCount / totalFiles) * 100);

    const progressContainer = document.getElementById('upload-progress-w');
    const progressBar = document.getElementById('upload-progress');

    if (progressContainer && progressBar) {
      progressContainer.classList.remove('progress-hidden');
      progressBar.value = overallProgress;
      progressBar.innerText = `${overallProgress}%`;
    }

    // Update counts
    successElement.innerText = uploadedCount;
    failedElement.innerText = failedCount;
  };

  // Switch over to progress view
  uploadContainer.classList.add('is-hidden');
  progressContainer.classList.remove('is-hidden');
  appendValidationWarning(errorsContainer, filtered.rejected);

  const fileChunks = chunkArray(filtered.allowed, CHUNK_SIZE);

  const uploadSingleFile = async (file, uploadToken) => {
    return await remoteUploadPhoto(
      prepareUploadUrl,
      commitUploadUrl,
      uploadToken,
      file,
    )
      .then((res) => {
        if (res.html) {
          galleryContainer.appendChild(createDomElement(res.html));
        }

        uploadedCount++;
        updateOverallProgress();

        return { ok: true, res };
      })
      .catch((err) => {
        console.error(err);

        failedCount++;
        updateOverallProgress();

        let errorMessage =
          '<p class="has-text-danger">Failed to upload photo</div>';

        if (err.response.data) {
          if (typeof err.response.data === 'string') {
            errorMessage = err.response.data;
          } else if (
            typeof err.response.data === 'object' &&
            err.response.data.message
          ) {
            errorMessage = `<p class="has-text-danger">${err.response.data.message}</div>`;
          }
        }

        errorsContainer.appendChild(createDomElement(errorMessage));

        return { ok: false };
      });
  };

  for (const fileChunk of fileChunks) {
    const results = await Promise.all(
      fileChunk.map((file) => uploadSingleFile(file, token)),
    );

    for (let i = results.length - 1; i >= 0; i--) {
      const result = results[i];
      if (result.ok && result.res.nextToken) {
        token = result.res.nextToken;
        break;
      }
    }
  }
}

document.addEventListener('change', (e) => {
  if (e.target.closest('#photos-input') || e.target.closest('#files-input')) {
    handleFilesSelect(e);
  }
});

document.addEventListener('click', (e) => {
  if (
    e.target.closest('#btn-upload-photos') ||
    e.target.closest('#btn-upload-files')
  ) {
    startUpload();
    e.preventDefault();
  }
});
