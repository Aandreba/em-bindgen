/** 
 * @param {string} accept
 * @param {boolean} multiple
 * @returns {Promise<File[] | null>}
 */
async function pickFiles(accept, multiple) {
	return new Promise(function (resolve) {
		const input = document.createElement("input");
		input.type = "file";
		input.accept = accept;
		input.multiple = multiple;
		input.addEventListener(
			"change",
			function () { resolve(input.files ? [...input.files] : null) },
			{ once: true, capture: true }
		);

		const dialog = document.createElement("dialog");
		dialog.addEventListener(
			"close",
			function () { resolve(null) },
			{ once: true, capture: true }
		);

		dialog.appendChild(input);
		dialog.showModal();
	});
}

/**
 * @param {FileSystemWriteChunkType & BlobPart} contents
 * @param {string} suggestedName
 * @param {string} suggestedMime
 * @param {object} types
 * @returns {boolean}
 */
async function saveFile(contents, suggestedName, suggestedMime, types) {
	try {
		if ("showSaveFilePicker" in window) {
			/** @type {FileSystemFileHandle} */
			let fileHandle;
			try {
				fileHandle = await window.showSaveFilePicker({ suggestedName, types });
			} catch (e) {
				if (e instanceof DOMException && (e.name == "AbortError" || e.code == DOMException.ABORT_ERR))
					return false;
				throw e;
			}

			const writableHandle = await fileHandle.createWritable();
			try {
				await writableHandle.write(contents);
			} finally {
				await writableHandle.close();
			}
		} else {
			const blob = new Blob([contents], { type: suggestedMime });
			const url = URL.createObjectURL(blob);
			try {
				const anchor = document.createElement("a");
				anchor.href = url;
				anchor.download = suggestedName;
				anchor.click();
			} finally {
				URL.revokeObjectURL(url);
			}
		}
	} catch (e) {
		console.error(e);
		return false;
	}
	return true;
}
