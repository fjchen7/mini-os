DOCKER_TAG ?= rcore-tutorial-v3:latest

DOCKER_CMD := docker run --rm -it -v ${PWD}:/mnt -w /mnt/os --name rcore-tutorial-v3 ${DOCKER_TAG}

run:
	${DOCKER_CMD} bash -c "LOG=TRACE make run"

dbgserver:
	${DOCKER_CMD} bash -c "LOG=TRACE make dbgserver"

dbgclient:
	${DOCKER_CMD} bash -c "LOG=TRACE make dbgclient"

docker:
	${DOCKER_CMD} bash

build_docker:
	docker build -t ${DOCKER_TAG} --target build .

fmt:
	cd os ; cargo fmt;  cd ..

.PHONY: docker build_docker fmt run dbgserver dbgclient
