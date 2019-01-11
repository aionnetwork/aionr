#!/usr/bin/env groovy

def message, lastCommit

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
		JAVA_ARGS="-Dorg.apache.commons.jelly.tags.fmt.timeZone=Asia/Shanghai"
		JENKINS_JAVA_OPTIONS="-Dorg.apache.commons.jelly.tags.fmt.timeZone=Asia/Shanghai"
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
                echo "building..."
                sh 'RUSTFLAGS="-D warnings" cargo build --release' 

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
							sh "python scripts/bench.py -l test_results/ut_result.txt -r test_results/report.html -c ${lastCommit}"
						}
						catch(Exception e){
							echo "${e}"
							throw e
						}
					}
					sh 'rm -rf $HOME/.aion/chains'	
			}
		}
		stage('RPC Test'){
			steps{
				sh 'set -e'
				script{
					try{
						sh './scripts/run_RPCtest.sh'
						sh 'echo $?'
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
			archiveArtifacts artifacts: 'target/release/aion,test_results/*.*',fingerprint:true
            slackSend channel: '#ci',
                      color: 'good',
                      message: "${currentBuild.fullDisplayName} completed successfully. Grab the generated builds at ${env.BUILD_URL}\nArtifacts: ${env.BUILD_URL}artifact/\n Check BenchTest result: ${env.BUILD_URL}artifact/test_results/report.html \nCommit: ${GIT_COMMIT}\nChanges:${message}"
        }
		
        failure {
            //cleanWs();
            slackSend channel: '#ci',
            color: 'danger', 
            message: "${currentBuild.fullDisplayName} failed at ${env.BUILD_URL}\nCommit: ${GIT_COMMIT}\nChanges:${message}"
        }
    }
}
