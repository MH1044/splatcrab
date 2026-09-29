% covers: 13 - fprintf(2, ...) writes to stderr through Interp.err and nothing to stdout, and the script ends with exit 0 (QA D25)
fprintf(2, 'to stderr\n')
