#!/usr/bin/env node

const { spawn } = require('child_process');
const chalk = require('chalk');
const ora = require('ora');

async function main() {
    const args = process.argv.slice(2);
    
    if (args.length === 0) {
        console.log(chalk.yellow('智能混合搜索引擎 CLI 工具'));
        console.log(chalk.gray('用法: smart-search <查询或URL> [选项]'));
        console.log('');
        console.log('选项:');
        console.log('  --verbose, -v    显示详细决策日志');
        console.log('  --quiet, -q      静默模式，仅输出结果');
        console.log('  --raw, -r        输出原始JSON响应');
        process.exit(1);
    }
    
    const query = args[0];
    const verbose = args.includes('--verbose') || args.includes('-v');
    const quiet = args.includes('--quiet') || args.includes('-q');
    const raw = args.includes('--raw') || args.includes('-r');
    
    // 构建混合搜索命令参数
    const hybridPath = '../skills/hybrid_search/target/release/hybrid_search.exe';
    const spinner = ora('正在处理搜索请求...').start();
    
    try {
        const child = spawn(hybridPath, [query], {
            cwd: __dirname
        });
        
        let stdout = '';
        let stderr = '';
        
        child.stdout.on('data', (data) => {
            stdout += data.toString();
        });
        
        child.stderr.on('data', (data) => {
            stderr += data.toString();
        });
        
        const exitCode = await new Promise((resolve) => {
            child.on('close', resolve);
        });
        
        spinner.stop();
        
        if (exitCode === 0) {
            if (raw) {
                console.log(stdout);
                return;
            }
            
            try {
                const response = JSON.parse(stdout);
                displayFormattedResponse(response, verbose);
            } catch (error) {
                console.log(chalk.red('❌ 响应解析失败'));
                console.log(stdout);
            }
        } else {
            console.log(chalk.red(`❌ 执行失败: ${stderr}`));
        }
    } catch (error) {
        spinner.stop();
        console.log(chalk.red(`❌ 启动失败: ${error.message}`));
    }
}

function displayFormattedResponse(response, verbose) {
    if (response.status === 'success') {
        console.log(chalk.green('✅ 搜索成功!'));
        
        // 显示内容或结果
        if (response.content) {
            console.log('\n' + response.content);
        } else if (response.results && response.results.length > 0) {
            console.log(`\n找到 ${response.results.length} 个结果:`);
            response.results.forEach((result, index) => {
                console.log(`\n${index + 1}.`);
                console.log(`   标题: ${chalk.bold(result.title)}`);
                console.log(`   链接: ${chalk.blue(result.url)}`);
                if (result.content) {
                    console.log(`   摘要: ${result.content}`);
                }
            });
        }
    } else {
        console.log(chalk.red('❌ 搜索失败!'));
        if (response.error) {
            console.log(`错误: ${chalk.red(response.error)}`);
        }
    }
    
    // 显示决策日志（如果启用）
    if (verbose && response.decision_log) {
        const log = response.decision_log;
        console.log(`\n${chalk.cyan.bold('📊 决策分析:')}`);
        console.log(`   查询类型: ${log.query_analyzed_as}`);
        console.log(`   最终选择: ${log.final_choice}`);
        console.log(`   置信度: ${(log.confidence * 100).toFixed(1)}%`);
        
        if (log.reasoning && log.reasoning.length > 0) {
            console.log('   推理过程:');
            log.reasoning.forEach(reason => {
                console.log(`     • ${reason}`);
            });
        }
        
        if (log.alternative_results && log.alternative_results.length > 0) {
            console.log('   备选方案:');
            log.alternative_results.forEach(alt => {
                console.log(`     • ${alt.skill}: ${alt.status} (${alt.reason})`);
            });
        }
    }
    
    // 显示执行时间
    if (response.duration_ms) {
        console.log(`\n⏱️  执行时间: ${response.duration_ms}ms`);
    }
}

main().catch(console.error);