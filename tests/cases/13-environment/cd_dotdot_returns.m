% covers: 15 - cd into a folder and cd .. come back to the folder pwd first gave
p = pwd; cd envdir; cd ..; disp(strcmp(pwd, p))
