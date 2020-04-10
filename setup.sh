#!/bin/bash

cd db/index
diesel database setup
diesel migration run

cd ../..

cd db/state
diesel database setup
diesel migration run

cd ../..