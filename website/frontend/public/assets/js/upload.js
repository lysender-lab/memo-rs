(function () {
  if (!window.X_UPLOAD_PHOTOS_EVENTS) {
    window.X_UPLOAD_PHOTOS_EVENTS = true;

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

    async function uploadPhoto(action, token, file, onUploadProgress) {
      const url = `${action}?token=${token}`;

      const config = {
        headers: {
          'Content-Type': 'multipart/form-data',
        },
        onUploadProgress,
      };
      const formData = new FormData();
      formData.append('file', file);

      const res = await axios.post(url, formData, config);
      return {
        nextToken: res.headers['x-next-token'],
        html: res.data,
      };
    }

    async function uploadPhotos() {
      const form = document.getElementById('upload-photos-form');
      const photosInput = document.getElementById('photos-input');
      const tokenInput = document.getElementById('upload-photos-token');
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
      const action = form.action;

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

      // Wanted to upload batch of 4 but concurrency is not good
      // in the backend side due to sqlite locking
      for (const file of files) {
        await uploadPhoto(action, token, file)
          .then((res) => {
            if (res.nextToken) {
              token = res.nextToken;
            }
            if (res.html) {
              galleryContainer.appendChild(createDomElement(res.html));
            }

            uploadedCount++;
            updateOverallProgress();
          })
          .catch((err) => {
            failedCount++;
            updateOverallProgress();
            if (err.response && err.response.data) {
              errorsContainer.appendChild(createDomElement(err.response.data));
            } else {
              errorsContainer.appendChild(
                createDomElement(
                  `<p class="has-text-danger">Failed to upload photo</div>`,
                ),
              );
            }
          });
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
  }
})();
