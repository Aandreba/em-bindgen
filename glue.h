#ifdef __cplusplus
extern "C" {
#endif

typedef struct _EM_VAL *GLUE_EM_VAL;

GLUE_EM_VAL glue_get_global(const char *name);
void glue_destroy_value(GLUE_EM_VAL obj);

#ifdef __cplusplus
}
#endif
