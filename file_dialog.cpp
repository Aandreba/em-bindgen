#include "file_dialog.h"
#include <emscripten.h>
#include <emscripten/val.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

File LoadFile(const char *accept, malloc_t memalloc) {
  File file;
  file.contents = nullptr;

  MAIN_THREAD_EM_ASM(
      {
        return Asyncify.handleAsync(async function() {
          const accept = UTF8ToString($0);
          const files = await new Promise(function(resolve) {
            const input = document.createElement("input");
            const dialog = document.createElement("dialog");

            input.type = "file";
            input.accept = accept;
            input.multiple = false;
            input.addEventListener("change",
                                   function() {
                                     resolve(input.files ? [... input.files]
                                                         : null);
                                     dialog.close();
                                   },
                                   {once : true, capture : true});

            dialog.addEventListener("close",
                                    function() {
                                      resolve(null);
                                      document.body.removeChild(dialog);
                                    },
                                    {once : true, capture : true});

            document.body.appendChild(dialog);
            dialog.appendChild(input);
            dialog.showModal();
          });

          if (files == null || files.length == 0)
            return;

          const file = files[0];
          Module.HEAPF64[$3 >> 3] = file.lastModified;

          const contents = new Uint8Array(await file.arrayBuffer());
          Module.HEAPU32[$2 >> 2] = contents.byteLength;

          const contentsPtr =
              Module.ccall("__INTERNAL_MALLOC_", "number",
                           [ "number", "number" ], [ $4, contents.byteLength ]);
          Module.HEAPU32[$1 >> 2] = contentsPtr;
          Module.HEAPU8.subarray(contentsPtr, contentsPtr + contents.byteLength)
              .set(contents);
        });
      },
      accept, &file.contents, &file.contents_len, &file.last_modified_ms,
      memalloc);

  return file;
}

bool SaveFile(const uint8_t *contents, uintptr_t contents_len,
              const char *suggested_name, const char *suggested_mime,
              const FileType *types, uintptr_t types_len) {
  return MAIN_THREAD_EM_ASM_INT(
             {
               return Asyncify.handleAsync(async function() {
                 const contents = Module.HEAPU8.slice($0, $0 + $1);
                 const suggestedName = ($2 == 0) ? undefined : UTF8ToString($2);
                 const suggestedMime = ($3 == 0) ? undefined : UTF8ToString($3);
                 const types = [];

                 try {
                   if ("showSaveFilePicker" in window) {
                     return await new Promise(function(resolve) {
                       const button = document.createElement("button");
                       const dialog = document.createElement("dialog");

                       button.innerHTML = "Save file";
                       button.addEventListener(
                           "click",
                           async function() {
                             try {
                               /** @type {FileSystemFileHandle} */
                               let fileHandle;
                               try {
                                 fileHandle = await window.showSaveFilePicker(
                                     {suggestedName, types});
                               } catch (e) {
                                 if (e instanceof
                                     DOMException &&
                                         (e.name == "AbortError" ||
                                          e.code == DOMException.ABORT_ERR))
                                   return resolve(false);
                                 throw e;
                               }

                               const writableHandle =
                                   await fileHandle.createWritable();
                               try {
                                 await writableHandle.write(contents);
                               } finally {
                                 await writableHandle.close();
                               }

                               resolve(true);
                             } finally {
                               dialog.close();
                             }
                           },
                           {once : true, capture : true});

                       dialog.addEventListener("close",
                                               function() {
                                                 resolve(false);
                                                 document.body.removeChild(
                                                     dialog);
                                               },
                                               {once : true, capture : true});

                       document.body.appendChild(dialog);
                       dialog.appendChild(button);
                       dialog.showModal();
                     });
                   } else {
                     const blob = new Blob([contents], {
                       type:
                         suggestedMime
                     });
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
                   return 0;
                 }
                 return 1;
               });
             },
             contents, contents_len, suggested_name, suggested_mime, types,
             types_len) != 0;
}

EMSCRIPTEN_KEEPALIVE void *__INTERNAL_MALLOC_(malloc_t memalloc,
                                              uintptr_t len) {
  return (memalloc)(len);
}

#ifdef __cplusplus
}
#endif