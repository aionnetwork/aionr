#!/bin/sh
# generate documentation only for aion libraries

cargo doc --no-deps --verbose --all &&
	echo '<meta http-equiv=refresh content=0;url=acore/index.html>' > target/doc/index.html
