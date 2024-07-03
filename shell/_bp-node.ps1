
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'bp-node' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'bp-node'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'bp-node' {
            [CompletionResult]::new('-w', 'w', [CompletionResultType]::ParameterName, 'w')
            [CompletionResult]::new('--wallet', 'wallet', [CompletionResultType]::ParameterName, 'wallet')
            [CompletionResult]::new('-W', 'W ', [CompletionResultType]::ParameterName, 'Path to wallet directory')
            [CompletionResult]::new('--wallet-path', 'wallet-path', [CompletionResultType]::ParameterName, 'Path to wallet directory')
            [CompletionResult]::new('--wpkh', 'wpkh', [CompletionResultType]::ParameterName, 'Use wpkh(KEY) descriptor as wallet')
            [CompletionResult]::new('--tr-key-only', 'tr-key-only', [CompletionResultType]::ParameterName, 'Use tr(KEY) descriptor as wallet')
            [CompletionResult]::new('--electrum', 'electrum', [CompletionResultType]::ParameterName, 'Electrum server to use')
            [CompletionResult]::new('--esplora', 'esplora', [CompletionResultType]::ParameterName, 'Esplora server to use')
            [CompletionResult]::new('-d', 'd', [CompletionResultType]::ParameterName, 'Data directory path')
            [CompletionResult]::new('--data-dir', 'data-dir', [CompletionResultType]::ParameterName, 'Data directory path')
            [CompletionResult]::new('-n', 'n', [CompletionResultType]::ParameterName, 'Network to use')
            [CompletionResult]::new('--network', 'network', [CompletionResultType]::ParameterName, 'Network to use')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Set verbosity level')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Set verbosity level')
            [CompletionResult]::new('--sync', 'sync', [CompletionResultType]::ParameterName, 'sync')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('-V', 'V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
