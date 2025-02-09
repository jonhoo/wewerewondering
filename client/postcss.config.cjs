import tailwind from "@tailwindcss/postcss";
import tailwindConfig from "./tailwind.config.cjs";
import autoprefixer from "autoprefixer";

export default {
	plugins: [tailwind(tailwindConfig), autoprefixer]
};
