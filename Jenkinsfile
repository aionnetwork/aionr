#!/usr/bin/env groovy

def message, lastCommit,tag

@NonCPS
def getCommit(){
	def changeLogSets = currentBuild.changeSets
	def m = "";
	for (int i = 0; i < changeLogSets.size(); i++) {
		def entries = changeLogSets[i].items
		for (int j = 0; j < entries.length; j++) {
			def entry = entries[j]
			m = "${m}\n${entry.commitId} by ${entry.author} on ${new Date(entry.timestamp)}:\n\t${entry.msg}"
		}
	}
	return m
}

pipeline {
    agent any

    triggers {
        cron('H 23 * * *')
        pollSCM('H/5 * * * *')
    }
	environment{
		JAVA_HOME="/run/jdk-11"
		ANT_HOME="/run/apache-ant-1.10.5"
		PATH="${JAVA_HOME}/bin:${ANT_HOME}/bin:${PATH}"
		LIBRARY_PATH="${JAVA_HOME}/lib/server"
		LD_LIBRARY_PATH="${LIBRARY_PATH}:/usr/local/lib:/run/libs"
	}


    options {
        timeout(time: 120, unit: 'MINUTES') 
	buildDiscarder(logRotator(numToKeepStr: '10'))
	disableConcurrentBuilds()
    }
    stages {
        stage('Format_Test') {
            steps {
                sh 'set -e'
                echo "format testing..."
                sh 'cargo +nightly fmt --all -- --check'
            }
        }
        stage('Build'){
            steps{
            	sh 'set -e'
                echo "clean old package"
            	sh 'rm aionr*.tar.gz || echo "no previous build packages"'
            	sh 'rm -r package || echo "no previous build package folder"'
            	echo 'clean compiled version.rs'
            	sh 'rm -r target/release/build/aion-version* target/release/build/avm-* || echo "no aion-version folders exist"'
            	echo "building..."
                sh 'RUSTFLAGS="-D warnings" ./resources/package.sh "aionr-$(git describe --abbrev=0)-$(date +%Y%m%d)"'
            }
        }
		stage('Unit Test'){
			steps{
					sh 'ls test_results || mkdir test_results'
					sh 'RUSTFLAGS="-D warnings" cargo +nightly test --all --no-run --release --exclude fastvm --exclude solidity'
					
					script{
						try{
							sh '''#!/bin/bash
							set -o pipefail
							RUSTFLAGS="-D warnings" cargo +nightly test  --all --release -- --nocapture --test-threads 1 2>&1 | tee test_results/ut_result.txt'''
							sh 'echo $?'
							lastCommit = sh(returnStdout: true, script: 'git rev-parse HEAD | cut -c 1-8')
							echo "${lastCommit}"
							sh "python resources/bench.py -l test_results/ut_result.txt -r test_results/report.html -c ${lastCommit}"
						}
						catch(Exception e){
							echo "${e}"
							throw e
						}
					}
					
			}
		}
		stage('RPC Test'){
			steps{
				sh 'set -e'
				script{
					try{
						sh './resources/run_RPCtest.sh'
					}
					catch(Exception e){
						echo "${e}"
						
						throw e
					}
				}
			}
		}
    }
    post{
        always{
        	script {
				//a GString like "${my_var}" and some class expects String. It can't be cast automatically.
				//If you have some code like this, you have to convert it to String like this: "${my_var}".toString()
				message = getCommit().toString();
			}
			
        }

        success{
			archiveArtifacts artifacts: '*.tar.gz,test_results/*.*,target/release/aion',fingerprint:true
            slackSend channel: '#shanghai_ci',
                      color: 'good',
                      message: "${currentBuild.fullDisplayName} completed successfully. Grab the generated builds at ${env.BUILD_URL}\nArtifacts: ${env.BUILD_URL}artifact/\n Check BenchTest result: ${env.BUILD_URL}artifact/test_results/report.html \nCommit: ${GIT_COMMIT}\nChanges:${message}"
        }
		
        failure {
            //cleanWs();
            slackSend channel: '#shanghai_ci',
            color: 'danger', 
            message: "${currentBuild.fullDisplayName} failed at ${env.BUILD_URL}\nCommit: ${GIT_COMMIT}\nChanges:${message}"
        }
    }
}
