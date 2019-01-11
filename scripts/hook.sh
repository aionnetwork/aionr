#!/bin/sh
FILE=./.git/hooks/pre-commit

cp ./.githooks/pre-commit $FILE
chmod +x $FILE
