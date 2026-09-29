% covers: 7 - clock is a 1x6 date vector, version is text, and getenv of an unset variable is empty (machine-dependent values are never pinned)
c = clock; disp(size(c)); disp(ischar(version)); disp(isempty(getenv('SPLATCRAB_NO_SUCH_VARIABLE')))
