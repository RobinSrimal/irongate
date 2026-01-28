import * as cdk from "aws-cdk-lib";
import { Construct } from "constructs";
import * as dynamodb from "aws-cdk-lib/aws-dynamodb";
import * as apigateway from "aws-cdk-lib/aws-apigatewayv2";
import * as integrations from "aws-cdk-lib/aws-apigatewayv2-integrations";
import { RustFunction } from "cargo-lambda-cdk";

export interface IrongateStackProps extends cdk.StackProps {
  /**
   * Enable development mode (allows localhost redirect URIs)
   * @default false
   */
  devMode?: boolean;
}

export class IrongateStack extends cdk.Stack {
  public readonly api: apigateway.HttpApi;
  public readonly table: dynamodb.Table;
  public readonly authFunction: RustFunction;

  constructor(scope: Construct, id: string, props?: IrongateStackProps) {
    super(scope, id, props);

    const devMode = props?.devMode ?? false;

    // DynamoDB Table
    this.table = new dynamodb.Table(this, "AuthTable", {
      partitionKey: { name: "pk", type: dynamodb.AttributeType.STRING },
      sortKey: { name: "sk", type: dynamodb.AttributeType.STRING },
      billingMode: dynamodb.BillingMode.PAY_PER_REQUEST,
      timeToLiveAttribute: "expiry",
      removalPolicy: cdk.RemovalPolicy.RETAIN,
    });

    // Rust Lambda Function (using cargo-lambda-cdk)
    this.authFunction = new RustFunction(this, "AuthFunction", {
      manifestPath: "../rust/Cargo.toml",
      architecture: cdk.aws_lambda.Architecture.ARM_64,
      memorySize: 256,
      timeout: cdk.Duration.seconds(30),
      environment: {
        DYNAMODB_TABLE: this.table.tableName,
        RUST_LOG: "info",
        // Security: Explicitly configure trusted proxies
        TRUSTED_PROXIES: "api-gateway",
        // Security: Dev mode disabled by default
        DEV_MODE: devMode ? "true" : "false",
      },
    });

    // Grant DynamoDB permissions
    this.table.grantReadWriteData(this.authFunction);

    // API Gateway
    this.api = new apigateway.HttpApi(this, "AuthApi", {
      apiName: "IrongateApi",
      description: "Irongate OAuth 2.0 Authorization Server",
    });

    // Default route to Lambda
    this.api.addRoutes({
      path: "/{proxy+}",
      methods: [apigateway.HttpMethod.ANY],
      integration: new integrations.HttpLambdaIntegration(
        "AuthIntegration",
        this.authFunction
      ),
    });

    // Root route
    this.api.addRoutes({
      path: "/",
      methods: [apigateway.HttpMethod.ANY],
      integration: new integrations.HttpLambdaIntegration(
        "RootIntegration",
        this.authFunction
      ),
    });

    // Outputs
    new cdk.CfnOutput(this, "ApiUrl", {
      value: this.api.url ?? "undefined",
      description: "Irongate API URL",
    });

    new cdk.CfnOutput(this, "TableName", {
      value: this.table.tableName,
      description: "DynamoDB Table Name",
    });
  }
}
