% covers: 12 - repmat whose tiled size is past the maximum array size is refused before anything is allocated, the message naming every size; exit 1, never an abort
repmat(1, 1e5, 1e5, 1e5)
