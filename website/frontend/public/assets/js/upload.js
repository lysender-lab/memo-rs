function handleFilesSelect(e) {
  const files = e.target.files;
  if (files.length > 0) {
    const label = document.getElementById('selected-files-label');
    if (label) {
      label.innerText = files.length + ' file(s) selected';
    }

    const container = document.getElementById('photos-input-w');
    if (container) {
      container.classList.add('is-success');
    }
  }
}

function showUploadFinished() {
  const elem = document.getElementById('h-uploading-photos');
  if (elem) {
    elem.innerHTML = 'Upload finished';
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

function chunkArray(items, chunkSize) {
  const chunks = [];

  for (let i = 0; i < items.length; i += chunkSize) {
    chunks.push(items.slice(i, i + chunkSize));
  }

  return chunks;
}

function startUploadPhotos() {
  uploadPhotos()
    .then(() => {
      showUploadFinished();
      showUploadMore();
    })
    .catch((_err) => {
      showUploadFinished();
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

async function uploadPhotos() {
  // Match chunks with server's core (2)
  const CHUNK_SIZE = 2;
  const form = document.getElementById('upload-photos-form');
  const photosInput = document.getElementById('photos-input');
  const tokenInput = document.getElementById('upload-photos-token');
  const prepareUploadInput = document.getElementById('prepare-upload-url');
  const galleryContainer = document.getElementById('photo-gallery');
  const uploadContainer = document.getElementById('photos-input-w');
  const progressContainer = document.getElementById('upload-progress-w');
  const errorsContainer = document.getElementById('progress-errors-w');
  const successElement = document.getElementById('progress-uploaded-count');
  const failedElement = document.getElementById('progress-failed-count');

  if (
    !form ||
    !uploadContainer ||
    !photosInput ||
    !tokenInput ||
    !galleryContainer ||
    !progressContainer ||
    !errorsContainer ||
    !successElement ||
    !failedElement
  ) {
    return;
  }

  const files = photosInput.files;
  const prepareUploadUrl = prepareUploadInput.value;
  const commitUploadUrl = form.action;

  // Token will change on every upload batch
  let token = tokenInput.value.toString();

  if (files.length === 0) {
    alert('Please select photos to upload');
    return;
  }

  const totalFiles = files.length;
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

  const filesArray = Array.from(files);
  const fileChunks = chunkArray(filesArray, CHUNK_SIZE);

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
  if (e.target.closest('#photos-input')) {
    handleFilesSelect(e);
  }
});

document.addEventListener('click', (e) => {
  if (e.target.closest('#btn-upload-photos')) {
    startUploadPhotos();
    e.preventDefault();
  }
});
